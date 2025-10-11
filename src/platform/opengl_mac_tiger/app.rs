use objc::runtime::{Class, Object, Sel, YES, NO};
use objc::{msg_send, sel};
use once_cell::sync::Lazy;
use core::ffi::{c_int, c_float, c_ulong, c_void};

macro_rules! use_classes {
    ($($class_name:ident),+ $(,)?) => {
        $(
            #[allow(non_upper_case_globals)]
            static $class_name: Lazy<&'static Class> = Lazy::new(|| {
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

#[link(name = "Cocoa", kind = "framework")]
extern "C" {
    #[link_name = "NSApp"]
    static ns_app: *mut Object;
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSPoint {
    pub x: c_float,
    pub y: c_float,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSSize {
    pub width: c_float,
    pub height: c_float,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSRect {
    pub origin: NSPoint,
    pub size: NSSize,
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

#[repr(c_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NSBackingStoreType {
    Retained = 0,
    Nonretained = 1,
    Buffered = 2,
}

#[repr(c_ulong)]
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
            msg_send![NSString,
                initWithBytes:self.as_ptr()
                length:self.len() as c_ulong
                encoding:NSStringEncoding::UTF8
            ];
        }
    }
}

/// `fn(key: &str) -> NSString`
unsafe fn ns_localized_string(key: &str) -> *mut Object {
    unsafe {
        msg_send![msg_send![NSBundle, mainBundle],
            localizedStringForKey:key.to_ns_string()
            value:"".to_ns_string()
            table:core::ptr::null_mut() as *mut Object
        ]
    }
}

#[repr(c_int)]
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

#[repr(c_int)]
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
}

pub unsafe fn run() {
    unsafe {
        let pool: *mut Object = msg_send![msg_send![NSAutoReleasePool, alloc], init];
        let _: () = msg_send![NSApplication, sharedApplication];
        let app_delegate: *mut Object =
            msg_send![msg_send![msg_send![AppDelegate, alloc], init], autorelease];
        let _: () = msg_send![ns_app, setDelegate:app_delegate];
        // We shouldn't actually get here, but do some cleanup if we do.
        let _: () = msg_send![pool, drain];
    }
}

#[allow(non_upper_case_globals)]
static AppDelegate: Lazy<&'static Class> = Lazy::new(|| unsafe {
    let mut decl = objc::declare::ClassDecl::new("AppDelegate", &NSObject).unwrap();

    // Instance variables

    // `NSWindow`
    decl.add_ivar::<*mut Object>("window");
    // `AppView`
    decl.add_ivar::<*mut Object>("view");

    // Methods

    /// `- (id) init`
    extern "C" fn init(this: &Object, _cmd: Sel) -> *mut Object {
        unsafe {
            let superclass = this.class().superclass().unwrap();
            let new_this: *mut Object = msg_send![super(this, superclass), init];
            if !new_this.is_null() {
                let screen_rect: NSRect = msg_send![msg_send![NSScreen, mainScreen], frame];
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
                    msg_send![NSWindow, alloc],
                    initWithContentRect:window_rect
                    styleMask:window_style
                    backing:NSBackingStoreType::Buffered
                    defer:YES
                ];
                let new_view: *mut Object = msg_send![
                    msg_send![AppView, alloc],
                    initWithFrame:content_rect
                ];
                (*new_this).set_ivar("window", new_window);
                (*new_this).set_ivar("view", new_view);
            }
            new_this
        }
    }
    decl.add_class_method(sel!(init), init);

    /// `- (void) applicationWillFinishLaunching:(NSNotification *) notification`
    extern "C" fn application_will_finish_launching(
        this: &Object,
        _cmd: Sel,
        notification: *mut Object,
    ) -> *mut Object {
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
        application_will_finish_launching,
    );

    /// `- (void) applicationDidFinishLaunching:(NSNotification *) notification`
    extern "C" fn application_did_finish_launching(
        this: &Object,
        _cmd: Sel,
        notification: *mut Object,
    ) -> *mut Object {
        unsafe {
            // Populate main and application menus
            {
                // Main menu
                let main_menu: *mut Object = msg_send![
                    msg_send![NSMenu, alloc],
                    initWithTitle:"MainMenu".to_ns_string()
                ];
                let menu_item: *mut Object = msg_send![main_menu,
                    addItemWithTitle:"Apple".to_ns_string()
                    action:core::ptr::null() as *const c_void
                    keyEquivalent:"".to_ns_string()
                ];
                let submenu: *mut Object = msg_send![msg_send![NSMenu, alloc],
                    initWithTitle:"Apple".to_ns_string()];
                let _: () = msg_send![ns_app, setAppleMenu:submenu];
                // Populate application menu
                {
                    // "About {APP_NAME}"
                    let app_name: *mut Object = msg_send![
                        msg_send![NSProcessInfo, processInfo],
                        processName
                    ];
                    let about_menu_item: *mut Object = msg_send![submenu,
                        addItemWithTitle:msg_send![
                            msg_send![
                                ns_localized_string("About"),
                                stringByAppendingString:" ".to_ns_string()
                            ],
                            stringByAppendingString:app_name
                        ]
                        action:sel!(orderFrontStandardAboutPanel:)
                        keyEquivalent:"".to_ns_string()
                    ];
                    let _: () = msg_send![about_menu_item, setTarget:ns_app];
                    // Separator
                    let _: () = msg_send![submenu, addItem:msg_send![NSMenuItem, separatorItem]];
                    // "Quit {APP_NAME}"
                    let quit_menu_item: *mut Object = msg_send![submenu,
                        addItemWithTitle:msg_send![
                            msg_send![
                                ns_localized_string("Quit"),
                                stringByAppendingString:" ".to_ns_string()
                            ],
                            stringByAppendingString:app_name
                        ]
                        action:sel!(terminate:)
                        keyEquivalent:"q".to_ns_string()
                    ];
                    let _: () = msg_send![quit_menu_item, setTarget:ns_app];
                }
                // Register menus
                let _: () = msg_send![main_menu, setSubmenu:submenu forItem:menu_item];
                let _: () = msg_send![ns_app, setMainMenu:main_menu];
            }
            // Make the window visible.
            let window: *mut Object = *this.get_ivar("window");
            let _: () = msg_send![window, makeKeyAndOrderFront:this];
        }
    }
    decl.add_method(
        sel!(applicationDidFinishLaunching:),
        application_did_finish_launching,
    );

    /// `- (void) dealloc`
    extern "C" fn dealloc(
        this: &Object,
        _cmd: Sel,
    ) -> *mut Object {
        unsafe {
            let superclass = this.class().superclass().unwrap();
            let window: *mut Object = *this.get_ivar("window");
            let view: *mut Object = *this.get_ivar("view");
            let _: () = msg_send![view, release];
            let _: () = msg_send![window, release];
            let _: () = msg_send![super(this, superclass), dealloc];
        }
    }
    decl.add_method(
        sel!(dealloc),
        dealloc,
    );

    decl.register()
});

#[allow(non_upper_case_globals)]
static AppView: Lazy<&'static Class> = Lazy::new(|| unsafe {
    let mut decl = objc::declare::ClassDecl::new("AppView", &NSOpenGLView).unwrap();

    // Instance variables

    decl.add_ivar::<CVDisplayLinkRef>("displayLink");
    decl.add_ivar::<bool>("running");
    // `*mut NSRecursiveLock`
    decl.add_ivar::<*mut Object>("appLock");

    // Methods

    /// `- (id) initWithFrame:(NSRect) frame`
    extern "C" fn init_with_frame(this: &Object, _cmd: Sel, frame: NSRect) -> *mut Object {
        unsafe {
            // No multisampling.
            let multisamples: c_int = 0;
            let opengl_attributes = {
                use NSOpenGLPixelFormatAttribute::*;
                #[rustfmt::skip]
                [
                    Multisample as c_int,
                    SampleBuffers as c_int, if multisamples > 0 { 1 } else { 0 },
                    Samples as c_int, multisamples,
                    Accelerated as c_int,
                    DoubleBuffer as c_int,
                    ColorSize as c_int, 32,
                    DepthSize as c_int, 24,
                    AlphaSize as c_int, 8,
                    End as c_int,
                ]
            };
            // Try to choose a supported pixel format.
            let pf: *mut Object = msg_send![msg_send![NSOpenGLPixelFormat, alloc],
                initWithAttributes:opengl_attributes
            ];
            if pf.is_null() {
                eprintln!("OpenGL pixel format not supported");
                return core::ptr::null_mut();
            }
            let superclass = this.class().superclass().unwrap();
            let new_this: *mut Object = msg_send![super(this, superclass),
                initWithFrame:frame
                pixelFormat:msg_send![pf, autorelease]
            ];
            let app_lock: *mut Object = msg_send![msg_send![NSRecursiveLock, alloc], init];
            (*new_this).set_ivar("appLock", app_lock);
            (*new_this).set_ivar("running", true);
            new_this
        }
    }
    decl.add_method(sel!(initWithFrame:), init_with_frame);

    /// `- (CVReturn) getFrameForTime:(const CVTimeStamp *) outputTime`
    extern "C" fn get_frame_for_time(this: &Object, _cmd: Sel, output_time: &CVTimeStamp) -> CVReturn {
        unsafe {
            draw_view(this);
            CVReturn::Success
        }
    }
    decl.add_method(sel!(getFrameForTime:), get_frame_for_time);

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
    decl.add_method(sel!(getFrameForTime:), get_frame_for_time);

    fn draw_view(this: &Object) {
        todo!()
    }

    decl.register()
});
