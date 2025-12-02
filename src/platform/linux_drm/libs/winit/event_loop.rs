// Much of this code is adapted from the `winit` source code.

use super::application::ApplicationHandler;
use super::dpi::PhysicalPosition;
use super::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, RawKeyEvent, StartCause, WindowEvent,
};
use super::keyboard::{KeyCode, NativeKeyCode, PhysicalKey};
use super::window::{CursorGrabMode, Window, WindowAttributes, WindowId, WindowInner};

use anyhow::Context;
use core::sync::atomic::{AtomicBool, Ordering};
use input::event::EventTrait;
use input::{AsRaw, Libinput, LibinputInterface};
use libseat::{Seat, SeatEvent};
use portable_std::FastHashMap;
use std::cell::{Cell, RefCell};
use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;

struct SeatInterfaceData {
    pub seat: Seat,
    pub fd_map: FastHashMap<RawFd, libseat::Device>,
}

#[derive(Clone)]
struct SeatInterface {
    pub data: Rc<RefCell<SeatInterfaceData>>,
}

impl LibinputInterface for SeatInterface {
    fn open_restricted(
        &mut self,
        path: &std::path::Path,
        _flags: i32,
    ) -> Result<std::os::fd::OwnedFd, i32> {
        let mut data = self.data.borrow_mut();
        let device = data
            .seat
            .open_device(&path)
            .map_err(|errno| i32::from(errno))?;
        let owned_fd = device
            .as_fd()
            .try_clone_to_owned()
            .map_err(|err| err.raw_os_error().unwrap_or(1))?;
        data.fd_map.insert(owned_fd.as_raw_fd(), device);
        Ok(owned_fd)
    }

    fn close_restricted(&mut self, fd: std::os::fd::OwnedFd) {
        let mut data = self.data.borrow_mut();
        let device = data.fd_map.remove(&fd.as_raw_fd()).unwrap();
        data.seat.close_device(device).unwrap();
    }
}

#[derive(Debug)]
enum SeatThreadEvent {
    InitFinished,
    TranslatedWindowEvent {
        window_id: WindowId,
        event: WindowEvent,
    },
    TranslatedDeviceEvent {
        device_id: DeviceId,
        event: DeviceEvent,
    },
    Error(anyhow::Error),
}

fn seat_thread(
    event_send_channel: mpsc::Sender<SeatThreadEvent>,
    window_recv_channel: mpsc::Receiver<Arc<WindowInner>>,
) {
    /// Attempt to send an event on `event_send_channel`.
    /// Return if `event_send_channel` has been closed.
    macro_rules! try_send_event_or_return {
        ($event:expr) => {
            match event_send_channel.send($event) {
                Ok(()) => {}
                Err(_) => return,
            }
        };
    }
    // Initialise udev.
    let seat_active = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut seat = {
        let seat_active = seat_active.clone();
        let seat = Seat::open(move |seat, event| match event {
            SeatEvent::Enable => {
                log::info!("Seat enabled - {}", seat.name());
                seat_active.set(true);
            }
            SeatEvent::Disable => {
                log::info!("Seat disabled - {}", seat.name());
                seat_active.set(false);
                seat.disable().unwrap();
            }
        })
        .context("Error while opening seat");
        match seat {
            Ok(seat) => seat,
            Err(err) => {
                try_send_event_or_return!(SeatThreadEvent::Error(err));
                return;
            }
        }
    };
    while !seat_active.get() {
        if let Err(err) = seat.dispatch(-1) {
            try_send_event_or_return!(SeatThreadEvent::Error(
                <Result<(), _>>::Err(err)
                    .context("Error during udev seat dispatch")
                    .unwrap_err()
            ));
            return;
        }
    }
    let seat_name = seat.name().to_owned();
    let seat_interface = SeatInterface {
        data: Rc::new(RefCell::new(SeatInterfaceData {
            seat,
            fd_map: FastHashMap::new(),
        })),
    };
    // Initialise libinput with our udev seat.
    let mut libinput = Libinput::new_with_udev(seat_interface.clone());
    if let Err(()) = libinput.udev_assign_seat(&seat_name) {
        try_send_event_or_return!(SeatThreadEvent::Error(anyhow::anyhow!(
            "Failed to assign udev {seat_name} to libinput"
        )));
        return;
    }
    // Notify the main thread that we're initialised.
    try_send_event_or_return!(SeatThreadEvent::InitFinished);
    // Pause libinput events until we've got a window.
    libinput.suspend();
    // Wait until a window has been registered for the event loop.
    let window_ = window_recv_channel.recv().unwrap();
    drop(window_recv_channel);
    let (window_width, window_height) = {
        let window = window_.data.lock().unwrap();
        (window.window_size.width, window.window_size.height)
    };
    // Now that we've got a window, start taking libinput events.
    if let Err(()) = libinput.resume() {
        try_send_event_or_return!(SeatThreadEvent::Error(anyhow::anyhow!(
            "Failed to resume libinput"
        )));
        return;
    }
    // Main event loop.
    {
        use rustix::event::{PollFd, PollFlags, poll};
        loop {
            // Wait for any events.
            {
                let mut seat_interface_data = seat_interface.data.borrow_mut();
                let seat_fd = match seat_interface_data.seat.get_fd() {
                    Ok(fd) => fd,
                    Err(err) => {
                        try_send_event_or_return!(SeatThreadEvent::Error(
                            <Result<(), _>>::Err(err)
                                .context("Error while getting libseat pollable fd")
                                .unwrap_err()
                        ));
                        return;
                    }
                };
                let mut polling_fds = [
                    PollFd::new(&libinput, PollFlags::IN),
                    PollFd::new(&seat_fd, PollFlags::IN),
                ];
                match poll(&mut polling_fds, None) {
                    Ok(_) => {}
                    Err(err) => try_send_event_or_return!(SeatThreadEvent::Error(
                        <Result<(), _>>::Err(err)
                            .context("Error while polling udev and libinput")
                            .unwrap_err()
                    )),
                }
            }
            // Collect udev events.
            {
                let mut seat_interface_data = seat_interface.data.borrow_mut();
                if let Err(err) = seat_interface_data.seat.dispatch(0) {
                    try_send_event_or_return!(SeatThreadEvent::Error(
                        <Result<(), _>>::Err(err)
                            .context("Error during udev dispatch")
                            .unwrap_err()
                    ))
                }
            }
            // Collect libinput events.
            if let Err(err) = libinput.dispatch() {
                try_send_event_or_return!(SeatThreadEvent::Error(
                    <Result<(), _>>::Err(err)
                        .context("Error during libinput dispatch")
                        .unwrap_err()
                ))
            }
            // Translate and send any pending libinput events.
            while let Some(event) = libinput.next() {
                match &event {
                    input::Event::Device(device_event) => match device_event {
                        input::event::DeviceEvent::Added(_) => {
                            // Turn off mouse acceleration.
                            // TODO: We should make this configurable in settings.
                            let mut device = event.device();
                            let is_likely_mouse = device
                                .has_capability(input::DeviceCapability::Pointer)
                                && device.touch_count().is_none();
                            if is_likely_mouse {
                                _ = device.config_accel_set_profile(input::AccelProfile::Flat);
                            }
                        }
                        _ => {}
                    },
                    input::Event::Pointer(pointer_event) => match pointer_event {
                        input::event::PointerEvent::Button(info) => {
                            let state = match info.button_state() {
                                input::event::pointer::ButtonState::Pressed => {
                                    ElementState::Pressed
                                }
                                input::event::pointer::ButtonState::Released => {
                                    ElementState::Released
                                }
                            };
                            let button = match info.button() {
                                // Found by experimentation.
                                // TODO: Where are these documented in libinput?
                                272 => MouseButton::Left,
                                273 => MouseButton::Right,
                                274 => MouseButton::Middle,
                                275 => MouseButton::Back,
                                276 => MouseButton::Forward,
                                _ => continue,
                            };
                            try_send_event_or_return!(SeatThreadEvent::TranslatedWindowEvent {
                                window_id: WindowId(()),
                                event: WindowEvent::MouseInput {
                                    // FIXME: Get device IDs.
                                    device_id: DeviceId::dummy(),
                                    state,
                                    button,
                                },
                            });
                        }
                        input::event::PointerEvent::Motion(info) => {
                            let delta = (info.dx(), info.dy());
                            // Update window cursor position.
                            let moved_cursor_pos: Option<PhysicalPosition<f64>> = {
                                let mut window_data = window_.data.lock().unwrap();
                                if window_data.cursor_grab_mode != CursorGrabMode::Locked {
                                    window_data.cursor_pos.x = f64::clamp(
                                        window_data.cursor_pos.x + delta.0,
                                        0.0,
                                        (window_width - 1) as f64,
                                    );
                                    window_data.cursor_pos.y = f64::clamp(
                                        window_data.cursor_pos.y + delta.1,
                                        0.0,
                                        (window_height - 1) as f64,
                                    );
                                    Some(window_data.cursor_pos)
                                } else {
                                    None
                                }
                            };
                            // FIXME: Get device IDs.
                            let device_id = DeviceId::dummy();
                            try_send_event_or_return!(SeatThreadEvent::TranslatedDeviceEvent {
                                device_id,
                                event: DeviceEvent::MouseMotion { delta },
                            });
                            if let Some(position) = moved_cursor_pos {
                                try_send_event_or_return!(SeatThreadEvent::TranslatedWindowEvent {
                                    window_id: WindowId(()),
                                    event: WindowEvent::CursorMoved {
                                        device_id,
                                        position,
                                    },
                                });
                            }
                        }
                        input::event::PointerEvent::MotionAbsolute(info) => {
                            let x = info.absolute_x_transformed(window_width);
                            let y = info.absolute_y_transformed(window_height);
                            let new_pos = PhysicalPosition { x, y };
                            // Update window cursor position.
                            {
                                let mut window_data = window_.data.lock().unwrap();
                                if window_data.cursor_grab_mode != CursorGrabMode::Locked {
                                    window_data.cursor_pos = new_pos;
                                }
                            }
                            try_send_event_or_return!(SeatThreadEvent::TranslatedWindowEvent {
                                window_id: WindowId(()),
                                event: WindowEvent::CursorMoved {
                                    // FIXME: Get device IDs.
                                    device_id: DeviceId::dummy(),
                                    position: new_pos,
                                },
                            });
                        }
                        _ => {}
                    },
                    input::Event::Keyboard(keyboard_event) => match keyboard_event {
                        input::event::KeyboardEvent::Key(info) => unsafe {
                            // XXX: The wrapper methods don't seem to have been implemented for
                            //      this yet, so we just grab the key information manually.
                            let raw_info = info.as_raw() as *mut _;
                            let raw_keycode = input::ffi::libinput_event_keyboard_get_key(raw_info);
                            let raw_state =
                                input::ffi::libinput_event_keyboard_get_key_state(raw_info);
                            let physical_key = libinput_keycode_to_physical_key(raw_keycode);
                            let state = match raw_state {
                                input::ffi::libinput_button_state_LIBINPUT_BUTTON_STATE_PRESSED => ElementState::Pressed,
                                input::ffi::libinput_button_state_LIBINPUT_BUTTON_STATE_RELEASED => ElementState::Released,
                                _ => unreachable!(),
                            };
                            try_send_event_or_return!(SeatThreadEvent::TranslatedDeviceEvent {
                                // FIXME: Get device IDs.
                                device_id: DeviceId::dummy(),
                                event: DeviceEvent::Key(RawKeyEvent {
                                    physical_key,
                                    state,
                                }),
                            });
                        },
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }
}

pub type ActiveEventLoop = EventLoop<()>;

pub struct EventLoop<T: 'static> {
    close_next_poll: AtomicBool,
    event_recv_channel: mpsc::Receiver<SeatThreadEvent>,
    pub(super) has_window: Cell<bool>,
    /// One-shot channel, receiver will hang up after receiving a window.
    pub(super) window_send_channel: mpsc::SyncSender<Arc<WindowInner>>,
    _user_data: T,
}

pub use anyhow::Error as EventLoopError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlFlow {
    Poll,
    #[default]
    Wait,
}

impl EventLoop<()> {
    pub fn new() -> Result<Self, EventLoopError> {
        let (event_send_channel, event_recv_channel) = mpsc::channel();
        let (window_send_channel, window_recv_channel) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            #[cfg(feature = "tracy")]
            tracing_tracy::client::set_thread_name!("Input Thread");
            seat_thread(event_send_channel, window_recv_channel)
        });
        // Wait for initialisation to finish.
        match event_recv_channel.recv().unwrap() {
            SeatThreadEvent::Error(err) => return Err(err),
            SeatThreadEvent::InitFinished => {}
            _ => unreachable!(),
        }
        Ok(Self {
            close_next_poll: AtomicBool::new(false),
            event_recv_channel,
            has_window: Cell::new(false),
            window_send_channel,
            _user_data: (),
        })
    }

    pub fn create_window(&self, window_attributes: WindowAttributes) -> anyhow::Result<Window> {
        Window::create(self, window_attributes)
    }

    pub fn exit(&self) {
        self.close_next_poll.store(true, Ordering::Relaxed);
    }

    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        assert!(control_flow == ControlFlow::Poll);
    }

    pub fn run_app<A: ApplicationHandler<()>>(self, app: &mut A) -> Result<(), EventLoopError> {
        app.new_events(&self, StartCause::Init);
        app.resumed(&self);
        loop {
            if self.close_next_poll.load(Ordering::Relaxed) {
                break Ok(());
            }
            for seat_thread_event in self.event_recv_channel.try_iter() {
                match seat_thread_event {
                    SeatThreadEvent::Error(err) => Err(err).context("Error on seat thread")?,
                    SeatThreadEvent::TranslatedWindowEvent { window_id, event } => {
                        app.window_event(&self, window_id, event)
                    }
                    SeatThreadEvent::TranslatedDeviceEvent { device_id, event } => {
                        app.device_event(&self, device_id, event)
                    }
                    unknown => unimplemented!("Unknown seat thread event - {unknown:?}"),
                }
            }
            app.new_events(&self, StartCause::Poll);
        }
    }
}

fn libinput_keycode_to_physical_key(keycode: u32) -> PhysicalKey {
    use KeyCode::*;
    PhysicalKey::Code(match keycode {
        1 => Escape,
        2 => Digit1,
        3 => Digit2,
        4 => Digit3,
        5 => Digit4,
        6 => Digit5,
        7 => Digit6,
        8 => Digit7,
        9 => Digit8,
        10 => Digit9,
        11 => Digit0,
        12 => Minus,
        13 => Equal,
        14 => Backspace,
        15 => Tab,
        16 => KeyQ,
        17 => KeyW,
        18 => KeyE,
        19 => KeyR,
        20 => KeyT,
        21 => KeyY,
        22 => KeyU,
        23 => KeyI,
        24 => KeyO,
        25 => KeyP,
        26 => BracketLeft,
        27 => BracketRight,
        28 => Enter,
        29 => ControlLeft,
        30 => KeyA,
        31 => KeyS,
        32 => KeyD,
        33 => KeyF,
        34 => KeyG,
        35 => KeyH,
        36 => KeyJ,
        37 => KeyK,
        38 => KeyL,
        39 => Semicolon,
        40 => Quote,
        41 => Backquote,
        42 => ShiftLeft,
        43 => Backslash,
        44 => KeyZ,
        45 => KeyX,
        46 => KeyC,
        47 => KeyV,
        48 => KeyB,
        49 => KeyN,
        50 => KeyM,
        51 => Comma,
        52 => Period,
        53 => Slash,
        54 => ShiftRight,
        55 => NumpadMultiply,
        56 => AltLeft,
        57 => Space,
        58 => CapsLock,
        59 => F1,
        60 => F2,
        61 => F3,
        62 => F4,
        63 => F5,
        64 => F6,
        65 => F7,
        66 => F8,
        67 => F9,
        68 => F10,
        69 => NumLock,
        70 => ScrollLock,
        71 => Numpad7,
        72 => Numpad8,
        73 => Numpad9,
        74 => NumpadSubtract,
        75 => Numpad4,
        76 => Numpad5,
        77 => Numpad6,
        78 => NumpadAdd,
        79 => Numpad1,
        80 => Numpad2,
        81 => Numpad3,
        82 => Numpad0,
        83 => NumpadDecimal,
        87 => F11,
        88 => F12,
        90 => Katakana,
        91 => Hiragana,
        96 => NumpadEnter,
        97 => ControlRight,
        98 => NumpadDivide,
        100 => AltRight,
        102 => Home,
        103 => ArrowUp,
        104 => PageUp,
        105 => ArrowLeft,
        106 => ArrowRight,
        107 => End,
        108 => ArrowDown,
        109 => PageDown,
        110 => Insert,
        111 => Delete,
        113 => AudioVolumeMute,
        114 => AudioVolumeDown,
        115 => AudioVolumeUp,
        117 => NumpadEqual,
        119 => Pause,
        121 => NumpadComma,
        124 => IntlYen,
        125 => SuperLeft,
        126 => SuperRight,
        142 => Sleep,
        183 => F13,
        184 => F14,
        185 => F15,
        186 => F16,
        187 => F17,
        188 => F18,
        189 => F19,
        190 => F20,
        191 => F21,
        192 => F22,
        193 => F23,
        194 => F24,
        _ => return PhysicalKey::Unidentified(NativeKeyCode::LibInput(keycode)),
    })
}
