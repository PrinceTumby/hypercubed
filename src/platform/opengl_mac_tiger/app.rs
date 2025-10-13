use crate::portable_prelude::eprintln;
use core::cell::{Cell, UnsafeCell};
use core::ffi::{c_float, c_int, c_ulong, c_void};
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, Ordering};
use objc::runtime::{Class, NO, Object, Sel, YES};
use objc::{Encode, Encoding, msg_send, sel};

struct LazySpin<T, F = fn() -> T> {
    value: UnsafeCell<MaybeUninit<T>>,
    /// Valid states:
    /// 0b00 - Uninitialised.
    /// 0b01 - Currently being initialised.
    /// 0b11 - Initialised.
    current_state: AtomicU8,
    init: Cell<Option<F>>,
}

unsafe impl<T, F: Send> Sync for LazySpin<T, F> {}

impl<T, F: FnOnce() -> T> LazySpin<T, F> {
    pub const fn new(init: F) -> Self {
        Self {
            value: UnsafeCell::new(MaybeUninit::uninit()),
            current_state: AtomicU8::new(0),
            init: Cell::new(Some(init)),
        }
    }

    pub fn force(this: &Self) -> &T {
        loop {
            let previous_state = this.current_state.fetch_or(0b01, Ordering::Acquire);
            match previous_state {
                0b00 => {
                    // If the value was previously uninitialised, and no threads are currently in the
                    // process of initialising the value, then we need to initialise it.
                    let init = this.init.take().unwrap();
                    unsafe {
                        *this.value.get() = MaybeUninit::new(init());
                    }
                    this.current_state.store(0b11, Ordering::Release);
                    return unsafe { (*this.value.get()).assume_init_ref() };
                }
                0b01 => {
                    // If the value is currently being initialised, we need to spin until it's
                    // done.
                    core::hint::spin_loop();
                }
                0b11 => {
                    // If the value has already been initialised, then we can just get a reference
                    // to it.
                    return unsafe { (*this.value.get()).assume_init_ref() };
                }
                _ => unreachable!(),
            }
        }
    }
}

impl<T, F: FnOnce() -> T> core::ops::Deref for LazySpin<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        Self::force(self)
    }
}

macro_rules! use_classes {
    ($($class_name:ident),+ $(,)?) => {
        $(
            #[allow(non_upper_case_globals)]
            static $class_name: LazySpin<&'static Class> = LazySpin::new(|| {
                ::objc::class!($class_name)
            });
        )+
    };
}

use_classes! {
    NSApplication,
    NSAutoReleasePool,
    NSBundle,
    NSMenu,
    NSMenuItem,
    NSNotification,
    NSObject,
    NSOpenGLPixelFormat,
    NSOpenGLView,
    NSProcessInfo,
    NSRecursiveLock,
    NSScreen,
    NSString,
    NSWindow,
}

/// Sends a message to an object, where the return type will be `*mut Object`.
macro_rules! msg_send_ret_obj {
    ($($any:tt)*) => {{
        let out: *mut ::objc::runtime::Object = ::objc::msg_send![$($any)*];
        out
    }};
}

macro_rules! unsafe_impl_encoding {
    (
        $( #[ $struct_meta:meta ] )*
        $struct_vis:vis struct $struct_name:ident {
            $(
                $( #[ $field_meta:meta ] )*
                $field_vis:vis $field_name:ident : $field_type:ty
            ),*
            $(,)?
        }
    ) => {
        $( #[ $struct_meta ] )*
        $struct_vis struct $struct_name {
            $(
                $( #[ $field_meta ] )*
                $field_vis $field_name : $field_type ,
            )*
        }

        unsafe impl Encode for $struct_name {
            const ENCODING: Encoding<'static> = Encoding::Struct(
                stringify!($struct_name),
                &[
                    $(<$field_type>::ENCODING,)*
                ],
            );
        }
    };
}

unsafe extern "C" {
    #[link_name = "NSApp"]
    unsafe static NS_APP: *mut Object;
}

unsafe_impl_encoding! {
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    struct NSPoint {
        pub x: c_float,
        pub y: c_float,
    }
}

unsafe_impl_encoding! {
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    struct NSSize {
        pub width: c_float,
        pub height: c_float,
    }
}

unsafe_impl_encoding! {
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    struct NSRect {
        pub origin: NSPoint,
        pub size: NSSize,
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    struct WindowStyle: c_int {
        const BORDERLESS = 0;
        const TITLED = 1 << 0;
        const CLOSABLE = 1 << 1;
        const MINIATURIZABLE = 1 << 2;
        const RESIZABLE = 1 << 3;
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NSBackingStoreType {
    Retained = 0,
    Nonretained = 1,
    Buffered = 2,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NSStringEncoding {
    ASCII = 1,
    NEXTSTEP = 2,
    UTF8 = 4,
}

trait ToNSString {
    /// `fn(&self) -> *mut NSString`
    fn to_ns_string(&self) -> *mut Object;
}

impl ToNSString for &str {
    fn to_ns_string(&self) -> *mut Object {
        unsafe {
            msg_send![
                *NSString,
                initWithBytes:self.as_ptr()
                length:self.len() as c_ulong
                encoding:NSStringEncoding::UTF8
            ]
        }
    }
}

/// `fn(key: &str) -> NSString`
unsafe fn ns_localized_string(key: &str) -> *mut Object {
    unsafe {
        msg_send![
            msg_send_ret_obj![*NSBundle, mainBundle],
            localizedStringForKey:key.to_ns_string()
            value:"".to_ns_string()
            table:core::ptr::null_mut() as *mut Object
        ]
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NSOpenGLPixelFormatAttribute {
    /// Marks the end of an attribute list.
    End = 0,
    DoubleBuffer = 5,
    /// `[ColorSize, <Number of color buffer bits>]`
    ColorSize = 8,
    /// `[AlphaSize, <Number of alpha component bits>]`
    AlphaSize = 11,
    /// `[DepthSize, <Number of depth buffer bits>]`
    DepthSize = 12,
    /// `[SampleBuffers, <Number of multisample buffers>]`
    SampleBuffers = 55,
    /// `[Samples, <Multisamples per multisample buffer>]`
    Samples = 56,
    Multisample = 59,
    Accelerated = 73,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NSOpenGLContextParameter {
    SwapInterval = 222,
}

type CVDisplayLinkRef = *mut c_void;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CVReturn {
    Success = 0,
}

unsafe impl Encode for CVReturn {
    const ENCODING: Encoding<'static> = Encoding::Int;
}

unsafe_impl_encoding! {
    #[repr(C)]
    struct CVSMPTETime {
        pub subframes: i16,
        pub subframe_divisor: i16,
        pub counter: u32,
        pub ty: u32,
        pub flags: u32,
        pub hours: i16,
        pub minutes: i16,
        pub seconds: i16,
        pub frames: i16,
    }
}

unsafe_impl_encoding! {
    #[repr(C)]
    struct CVTimeStamp {
        pub version: u32,
        /// Units per second.
        pub video_timescale: i32,
        /// Start of frame (or field for interlaced).
        pub video_time: i64,
        /// Host root timebase time.
        pub host_time: u64,
        /// Current rate (timestamps / nominal rate).
        pub rate_scalar: f64,
        /// Hint for nominal output rate.
        pub video_refresh_period: i64,
        pub flags: u64,
        reserved: u64,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
struct CVTimeStampPtr(pub *const CVTimeStamp);

unsafe impl objc::Encode for CVTimeStampPtr {
    const ENCODING: Encoding<'static> = Encoding::Pointer(&CVSMPTETime::ENCODING);
}

pub unsafe fn run() {
    unsafe {
        let pool: *mut Object = msg_send![msg_send_ret_obj![*NSAutoReleasePool, alloc], init];
        let _: () = msg_send![*NSApplication, sharedApplication];
        let app_delegate: *mut Object = msg_send![
            msg_send_ret_obj![msg_send_ret_obj![*AppDelegate, alloc], init],
            autorelease
        ];
        let _: () = msg_send![NS_APP, setDelegate:app_delegate];
        // We shouldn't actually get here, but do some cleanup if we do.
        let _: () = msg_send![pool, drain];
    }
}

#[allow(non_upper_case_globals)]
static AppDelegate: LazySpin<&'static Class> = LazySpin::new(|| unsafe {
    let mut decl = objc::declare::ClassDecl::new("AppDelegate", &NSObject).unwrap();

    // Instance variables

    // `NSWindow`
    decl.add_ivar::<*mut Object>("window");
    // `AppView`
    decl.add_ivar::<*mut Object>("view");

    // Methods

    /// `- (id) init`
    extern "C" fn init(mut this: &mut Object, _cmd: Sel) -> *mut Object {
        unsafe {
            let superclass = this.class().superclass().unwrap();
            let new_this: *mut Object = msg_send![super(this, superclass), init];
            if !new_this.is_null() {
                this = &mut *new_this;
                let screen_rect: NSRect =
                    msg_send![msg_send_ret_obj![*NSScreen, mainScreen], frame];
                let content_rect = NSRect {
                    origin: NSPoint { x: 0.0, y: 0.0 },
                    size: NSSize {
                        width: 800.0,
                        height: 600.0,
                    },
                };
                let window_rect = NSRect {
                    origin: NSPoint {
                        x: (screen_rect.size.width / 2.0) - (content_rect.size.width / 2.0),
                        y: (screen_rect.size.height / 2.0) - (content_rect.size.height / 2.0),
                    },
                    size: content_rect.size,
                };
                let window_style: c_int = (WindowStyle::TITLED
                    | WindowStyle::CLOSABLE
                    | WindowStyle::MINIATURIZABLE
                    | WindowStyle::RESIZABLE)
                    .bits();
                let new_window: *mut Object = msg_send![
                    msg_send_ret_obj![*NSWindow, alloc],
                    initWithContentRect:window_rect
                    styleMask:window_style
                    backing:NSBackingStoreType::Buffered
                    defer:YES
                ];
                let new_view: *mut Object = msg_send![
                    msg_send_ret_obj![*AppView, alloc],
                    initWithFrame:content_rect
                ];
                (*this).set_ivar("window", new_window);
                (*this).set_ivar("view", new_view);
            }
            new_this
        }
    }
    decl.add_method(
        sel!(init),
        init as extern "C" fn(&mut Object, Sel) -> *mut Object,
    );

    /// `- (void) applicationWillFinishLaunching:(NSNotification *) notification`
    extern "C" fn application_will_finish_launching(
        this: &Object,
        _cmd: Sel,
        _notification: *mut Object,
    ) {
        unsafe {
            // Attach the app view to the window.
            let window: *mut Object = *this.get_ivar("window");
            let view: *mut Object = *this.get_ivar("view");
            let _: () = msg_send![window, setContentView:view];
            let _: () = msg_send![window, setDelegate:view];
        }
    }
    decl.add_method(
        sel!(applicationWillFinishLaunching:),
        application_will_finish_launching as extern "C" fn(&Object, Sel, *mut Object),
    );

    /// `- (void) applicationDidFinishLaunching:(NSNotification *) notification`
    extern "C" fn application_did_finish_launching(
        this: &Object,
        _cmd: Sel,
        _notification: *mut Object,
    ) {
        unsafe {
            // Populate main and application menus
            {
                // Main menu
                let main_menu: *mut Object = msg_send![
                    msg_send_ret_obj![*NSMenu, alloc],
                    initWithTitle:"MainMenu".to_ns_string()
                ];
                let menu_item: *mut Object = msg_send![
                    main_menu,
                    addItemWithTitle:"Apple".to_ns_string()
                    action:core::ptr::null() as *const c_void
                    keyEquivalent:"".to_ns_string()
                ];
                let submenu: *mut Object = msg_send![
                    msg_send_ret_obj![*NSMenu, alloc],
                    initWithTitle:"Apple".to_ns_string()
                ];
                let _: () = msg_send![NS_APP, setAppleMenu:submenu];
                // Populate application menu
                {
                    // "About {APP_NAME}"
                    let app_name: *mut Object =
                        msg_send![msg_send_ret_obj![*NSProcessInfo, processInfo], processName];
                    let about_menu_item: *mut Object = msg_send![submenu,
                        addItemWithTitle:msg_send_ret_obj![
                            msg_send_ret_obj![
                                ns_localized_string("About"),
                                stringByAppendingString:" ".to_ns_string()
                            ],
                            stringByAppendingString:app_name
                        ]
                        action:sel!(orderFrontStandardAboutPanel:)
                        keyEquivalent:"".to_ns_string()
                    ];
                    let _: () = msg_send![about_menu_item, setTarget:NS_APP];
                    // Separator
                    let _: () = msg_send![
                        submenu,
                        addItem:msg_send_ret_obj![*NSMenuItem, separatorItem]
                    ];
                    // "Quit {APP_NAME}"
                    let quit_menu_item: *mut Object = msg_send![submenu,
                        addItemWithTitle:msg_send_ret_obj![
                            msg_send_ret_obj![
                                ns_localized_string("Quit"),
                                stringByAppendingString:" ".to_ns_string()
                            ],
                            stringByAppendingString:app_name
                        ]
                        action:sel!(terminate:)
                        keyEquivalent:"q".to_ns_string()
                    ];
                    let _: () = msg_send![quit_menu_item, setTarget:NS_APP];
                }
                // Register menus
                let _: () = msg_send![main_menu, setSubmenu:submenu forItem:menu_item];
                let _: () = msg_send![NS_APP, setMainMenu:main_menu];
            }
            // Make the window visible.
            let window: *mut Object = *this.get_ivar("window");
            let _: () = msg_send![window, makeKeyAndOrderFront:this];
        }
    }
    decl.add_method(
        sel!(applicationDidFinishLaunching:),
        application_did_finish_launching as extern "C" fn(&Object, Sel, *mut Object),
    );

    /// `- (void) dealloc`
    extern "C" fn dealloc(this: &Object, _cmd: Sel) {
        unsafe {
            let superclass = this.class().superclass().unwrap();
            let window: *mut Object = *this.get_ivar("window");
            let view: *mut Object = *this.get_ivar("view");
            let _: () = msg_send![view, release];
            let _: () = msg_send![window, release];
            let _: () = msg_send![super(this, superclass), dealloc];
        }
    }
    decl.add_method(sel!(dealloc), dealloc as extern "C" fn(&Object, Sel));

    decl.register()
});

#[allow(non_upper_case_globals)]
static AppView: LazySpin<&'static Class> = LazySpin::new(|| unsafe {
    let mut decl = objc::declare::ClassDecl::new("AppView", &NSOpenGLView).unwrap();

    // Instance variables

    decl.add_ivar::<CVDisplayLinkRef>("displayLink");
    decl.add_ivar::<bool>("running");
    // `*mut NSRecursiveLock`
    decl.add_ivar::<*mut Object>("appLock");

    // Methods

    /// `- (id) initWithFrame:(NSRect) frame`
    extern "C" fn init_with_frame(mut this: &mut Object, _cmd: Sel, frame: NSRect) -> *mut Object {
        unsafe {
            // No multisampling.
            let multisamples: c_int = 0;
            let opengl_attributes = {
                use NSOpenGLPixelFormatAttribute::*;
                [
                    Multisample as c_int,
                    SampleBuffers as c_int,
                    if multisamples > 0 { 1 } else { 0 },
                    Samples as c_int,
                    multisamples,
                    Accelerated as c_int,
                    DoubleBuffer as c_int,
                    ColorSize as c_int,
                    32,
                    DepthSize as c_int,
                    24,
                    AlphaSize as c_int,
                    8,
                    End as c_int,
                ]
            };
            // Try to choose a supported pixel format.
            let pf: *mut Object = msg_send![msg_send_ret_obj![*NSOpenGLPixelFormat, alloc],
                initWithAttributes:opengl_attributes
            ];
            if pf.is_null() {
                eprintln!("OpenGL pixel format not supported");
                return core::ptr::null_mut();
            }
            let superclass = this.class().superclass().unwrap();
            let new_this: *mut Object = msg_send![super(this, superclass),
                initWithFrame:frame
                pixelFormat:msg_send_ret_obj![pf, autorelease]
            ];
            this = &mut *new_this;
            let app_lock: *mut Object = msg_send![msg_send_ret_obj![*NSRecursiveLock, alloc], init];
            (*this).set_ivar("appLock", app_lock);
            (*this).set_ivar("running", true);
            this
        }
    }
    decl.add_method(
        sel!(initWithFrame:),
        init_with_frame as extern "C" fn(&mut Object, Sel, NSRect) -> *mut Object,
    );

    /// `- (CVReturn) getFrameForTime:(const CVTimeStamp *) outputTime`
    extern "C" fn get_frame_for_time(
        this: &Object,
        _cmd: Sel,
        _output_time: CVTimeStampPtr,
    ) -> CVReturn {
        unsafe {
            draw_view(this);
            CVReturn::Success
        }
    }
    decl.add_method(
        sel!(getFrameForTime:),
        get_frame_for_time as extern "C" fn(&Object, Sel, CVTimeStampPtr) -> CVReturn,
    );

    /// `- (void) prepareOpenGL
    extern "C" fn prepare_opengl(this: &Object, _cmd: Sel) {
        unsafe {
            let superclass = this.class().superclass().unwrap();
            let _: () = msg_send![super(this, superclass), prepareOpenGL];
            // Synchronise buffer swaps with VSync.
            let mut swap_interval: c_int = 1;
            let opengl_context: *mut Object = msg_send![this, openGLContext];
            let _: () = msg_send![opengl_context,
                setValues:&swap_interval
                forParameter:NSOpenGLContextParameter::SwapInterval
            ];
            todo!()
        }
    }
    decl.add_method(
        sel!(prepareOpenGL:),
        prepare_opengl as extern "C" fn(&Object, Sel),
    );

    fn draw_view(this: &Object) {
        todo!()
    }

    decl.register()
});
