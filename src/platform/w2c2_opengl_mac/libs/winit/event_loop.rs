// Much of this code is adapted from the `winit` source code.

use super::super::super::{HostU16, HostU32};
use super::dpi::PhysicalSize;
use super::event::{
    DeviceEvent, DeviceId, ElementState, Event, RawKeyEvent, StartCause, WindowEvent,
};
use super::keyboard::{KeyCode, NativeKeyCode, PhysicalKey};
use super::window::WindowId;

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use portable_std::Mutex;
use std::collections::VecDeque;

static PENDING_EVENTS_FRONT: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());
static PENDING_EVENTS_BACK: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());
pub static CURRENT_LOOP_COUNT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlFlow {
    Poll,
    #[default]
    Wait,
}

#[derive(Debug)]
pub struct EventLoop {
    close_next_poll: AtomicBool,
}

impl EventLoop {
    pub fn new() -> Result<Self, EventLoopError> {
        Ok(Self {
            close_next_poll: AtomicBool::new(false),
        })
    }

    pub fn exit(&self) {
        self.close_next_poll.store(true, Ordering::SeqCst);
    }

    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        assert!(control_flow == ControlFlow::Poll)
    }

    pub async fn run<F>(self, mut event_handler: F) -> Result<(), EventLoopError>
    where
        F: FnMut(Event, &Self),
    {
        loop {
            // Swap front and back event queues, so we can get the pending events while holding
            // the lock for as short a time as possible.
            {
                let mut front_lock = PENDING_EVENTS_FRONT.lock().unwrap();
                let mut back_lock = PENDING_EVENTS_BACK.lock().unwrap();
                std::mem::swap(&mut *front_lock, &mut *back_lock);
            }
            for event in PENDING_EVENTS_FRONT.lock().unwrap().drain(..) {
                event_handler(event, &self);
                if self.close_next_poll.load(Ordering::SeqCst) {
                    return Ok(());
                }
            }
            futures::future::poll_fn({
                let current_loop = CURRENT_LOOP_COUNT.load(Ordering::SeqCst);
                move |_ctx| {
                    if CURRENT_LOOP_COUNT.load(Ordering::SeqCst) > current_loop {
                        core::task::Poll::Ready(())
                    } else {
                        core::task::Poll::Pending
                    }
                }
            })
            .await;
        }
    }
}

#[derive(Debug)]
pub enum EventLoopError {}

impl core::fmt::Display for EventLoopError {
    fn fmt(&self, _f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        todo!()
    }
}

impl core::error::Error for EventLoopError {}

static mut CLIENT_NEXT_EXTERNAL_EVENT: MaybeUninit<ExternalEvent> = MaybeUninit::uninit();

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_get_next_external_event_ptr() -> *mut MaybeUninit<ExternalEvent> {
    &raw mut CLIENT_NEXT_EXTERNAL_EVENT
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_push_next_external_event() {
    let external_event = unsafe { CLIENT_NEXT_EXTERNAL_EVENT.assume_init() };
    PENDING_EVENTS_BACK
        .lock()
        .unwrap()
        .push_back(external_event.to_winit_event());
}

#[repr(C, u8)]
#[derive(Clone, Copy, Debug)]
pub enum ExternalEvent {
    PollNewEvents,
    WindowEvent(ExternalWindowEvent),
    DeviceEvent(ExternalDeviceEvent),
}

impl ExternalEvent {
    pub fn to_winit_event(self) -> Event {
        match self {
            Self::PollNewEvents => Event::NewEvents(StartCause::Poll),
            Self::WindowEvent(event) => Event::WindowEvent {
                window_id: WindowId(()),
                event: event.to_winit_window_event(),
            },
            Self::DeviceEvent(event) => Event::DeviceEvent {
                device_id: DeviceId::dummy(),
                event: event.to_winit_device_event(),
            },
        }
    }
}

// Window events

#[repr(C, u8)]
#[derive(Clone, Copy, Debug)]
pub enum ExternalWindowEvent {
    Resized { width: HostU32, height: HostU32 },
    CloseRequested,
}

impl ExternalWindowEvent {
    pub fn to_winit_window_event(self) -> WindowEvent {
        match self {
            Self::Resized { width, height } => WindowEvent::Resized(PhysicalSize {
                width: width.to_num(),
                height: height.to_num(),
            }),
            Self::CloseRequested => WindowEvent::CloseRequested,
        }
    }
}

// Device events

#[repr(C, u8)]
#[derive(Clone, Copy, Debug)]
pub enum ExternalDeviceEvent {
    Key(ExternalDeviceKeyEvent),
}

impl ExternalDeviceEvent {
    pub fn to_winit_device_event(self) -> DeviceEvent {
        match self {
            Self::Key(key_event) => DeviceEvent::Key(RawKeyEvent {
                physical_key: mac_scancode_to_physical_key(key_event.mac_scancode.to_num()),
                state: key_event.element_state.to_winit_element_state(),
            }),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ExternalDeviceKeyEvent {
    pub mac_scancode: HostU16,
    pub element_state: ExternalElementState,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalElementState {
    Pressed = 0,
    Released = 1,
}

impl ExternalElementState {
    pub fn to_winit_element_state(&self) -> ElementState {
        match self {
            Self::Pressed => ElementState::Pressed,
            Self::Released => ElementState::Released,
        }
    }
}

fn mac_scancode_to_physical_key(scancode: u16) -> PhysicalKey {
    PhysicalKey::Code(match scancode {
        0x00 => KeyCode::KeyA,
        0x01 => KeyCode::KeyS,
        0x02 => KeyCode::KeyD,
        0x03 => KeyCode::KeyF,
        0x04 => KeyCode::KeyH,
        0x05 => KeyCode::KeyG,
        0x06 => KeyCode::KeyZ,
        0x07 => KeyCode::KeyX,
        0x08 => KeyCode::KeyC,
        0x09 => KeyCode::KeyV,
        0x0A => KeyCode::IntlBackslash,
        0x0B => KeyCode::KeyB,
        0x0C => KeyCode::KeyQ,
        0x0D => KeyCode::KeyW,
        0x0E => KeyCode::KeyE,
        0x0F => KeyCode::KeyR,
        0x10 => KeyCode::KeyY,
        0x11 => KeyCode::KeyT,
        0x12 => KeyCode::Digit1,
        0x13 => KeyCode::Digit2,
        0x14 => KeyCode::Digit3,
        0x15 => KeyCode::Digit4,
        0x16 => KeyCode::Digit6,
        0x17 => KeyCode::Digit5,
        0x18 => KeyCode::Equal,
        0x19 => KeyCode::Digit9,
        0x1A => KeyCode::Digit7,
        0x1B => KeyCode::Minus,
        0x1C => KeyCode::Digit8,
        0x1D => KeyCode::Digit0,
        0x1E => KeyCode::BracketRight,
        0x1F => KeyCode::KeyO,
        0x20 => KeyCode::KeyU,
        0x21 => KeyCode::BracketLeft,
        0x22 => KeyCode::KeyI,
        0x23 => KeyCode::KeyP,
        0x24 => KeyCode::Enter,
        0x25 => KeyCode::KeyL,
        0x26 => KeyCode::KeyJ,
        0x27 => KeyCode::Quote,
        0x28 => KeyCode::KeyK,
        0x29 => KeyCode::Semicolon,
        0x2A => KeyCode::Backslash,
        0x2B => KeyCode::Comma,
        0x2C => KeyCode::Slash,
        0x2D => KeyCode::KeyN,
        0x2E => KeyCode::KeyM,
        0x2F => KeyCode::Period,
        0x30 => KeyCode::Tab,
        0x31 => KeyCode::Space,
        0x32 => KeyCode::Backquote,
        0x33 => KeyCode::Backspace,
        0x35 => KeyCode::Escape,
        0x38 => KeyCode::ShiftLeft,
        0x39 => KeyCode::CapsLock,
        0x3A => KeyCode::AltLeft,
        0x3B => KeyCode::ControlLeft,
        0x3C => KeyCode::ShiftRight,
        0x3D => KeyCode::AltRight,
        0x3E => KeyCode::ControlRight,
        0x3F => KeyCode::Fn,
        0x40 => KeyCode::F17,
        0x41 => KeyCode::NumpadDecimal,
        0x43 => KeyCode::NumpadMultiply,
        0x45 => KeyCode::NumpadAdd,
        0x47 => KeyCode::NumLock,
        0x48 => KeyCode::AudioVolumeUp,
        0x49 => KeyCode::AudioVolumeDown,
        0x4A => KeyCode::AudioVolumeMute,
        0x4B => KeyCode::NumpadDivide,
        0x4C => KeyCode::NumpadEnter,
        0x4E => KeyCode::NumpadSubtract,
        0x4F => KeyCode::F18,
        0x50 => KeyCode::F19,
        0x51 => KeyCode::NumpadEqual,
        0x52 => KeyCode::Numpad0,
        0x53 => KeyCode::Numpad1,
        0x54 => KeyCode::Numpad2,
        0x55 => KeyCode::Numpad3,
        0x56 => KeyCode::Numpad4,
        0x57 => KeyCode::Numpad5,
        0x58 => KeyCode::Numpad6,
        0x59 => KeyCode::Numpad7,
        0x5A => KeyCode::F20,
        0x5B => KeyCode::Numpad8,
        0x5C => KeyCode::Numpad9,
        0x5D => KeyCode::IntlYen,
        0x5E => KeyCode::IntlRo,
        0x5F => KeyCode::NumpadComma,
        0x60 => KeyCode::F5,
        0x61 => KeyCode::F6,
        0x62 => KeyCode::F7,
        0x63 => KeyCode::F3,
        0x64 => KeyCode::F8,
        0x65 => KeyCode::F9,
        0x66 => KeyCode::Lang2,
        0x67 => KeyCode::F11,
        0x68 => KeyCode::Lang1,
        0x69 => KeyCode::F13,
        0x6A => KeyCode::F16,
        0x6B => KeyCode::F14,
        0x6D => KeyCode::F10,
        0x6E => KeyCode::ContextMenu,
        0x6F => KeyCode::F12,
        0x71 => KeyCode::F15,
        0x72 => KeyCode::Insert,
        0x73 => KeyCode::Home,
        0x74 => KeyCode::PageUp,
        0x75 => KeyCode::Delete,
        0x76 => KeyCode::F4,
        0x77 => KeyCode::End,
        0x78 => KeyCode::F2,
        0x79 => KeyCode::PageDown,
        0x7A => KeyCode::F1,
        0x7B => KeyCode::ArrowLeft,
        0x7C => KeyCode::ArrowRight,
        0x7D => KeyCode::ArrowDown,
        0x7E => KeyCode::ArrowUp,
        _ => return PhysicalKey::Unidentified(NativeKeyCode::MacOS(scancode as u16)),
    })
}
