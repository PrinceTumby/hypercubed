#import <Cocoa/Cocoa.h>
#import <QuartzCore/CVDisplayLink.h>
#import <OpenGL/OpenGL.h>
#import <OpenGL/gl.h>

#import "w2c2_base.h"
#import "wasi.h"
#import "hypercubed.h"
#import "client_event.h"

static const float VERTICES[3][3] = {
    {0.0f, 0.5f, 0.0f},
    {-0.5f, -0.5f, 0.0f},
    {0.5f, -0.5f, 0.0f},
};
static const float VERTEX_COLOURS[3][3] = {
    {1.0f, 0.0f, 0.0f},
    {0.0f, 1.0f, 0.0f},
    {0.0f, 0.0f, 1.0f},
};

minecraftclientInstance client_instance;
ClientEvent *next_external_event_ptr = NULL;

wasmMemory *wasiMemory(void *instance) {
    return minecraftclient_memory((minecraftclientInstance *)instance);
}

void init_next_external_event_ptr(void) {
    U32 wasm_ptr = minecraftclient_client_get_next_external_event_ptr(&client_instance);
    wasmMemory *memory = wasiMemory(&client_instance);
    next_external_event_ptr = (void*)memory->data + wasm_ptr;
}

/// Custom OpenGL View
@class View;

static CVReturn GlobalDisplayLinkCallback(CVDisplayLinkRef, const CVTimeStamp*, const CVTimeStamp*, CVOptionFlags, CVOptionFlags*, void*);

// Custom Application delegate

@interface AppDelegate : NSObject
{
    NSWindow *window;
    View *view;
}

- (void)populateMainMenu;
- (void)populateApplicationMenu:(NSMenu *)menu;

@end

@implementation AppDelegate

- (id) init {
    if (self = [super init]) {
        NSRect screenRect = [[NSScreen mainScreen] frame];
        NSRect contentRect = NSMakeRect(0, 0, 800, 600);
        // NSRect contentRect = NSMakeRect(0, 0, screenRect.size.width, screenRect.size.height);
        NSRect windowRect = NSMakeRect(NSMidX(screenRect) - NSMidX(contentRect),
                                     NSMidY(screenRect) - NSMidY(contentRect),
                                     contentRect.size.width,
                                     contentRect.size.height);
        int windowStyle = NSTitledWindowMask
            | NSClosableWindowMask
            | NSResizableWindowMask;
        window = [[NSWindow alloc]
            initWithContentRect:windowRect
            styleMask:windowStyle
            backing:NSBackingStoreBuffered
            defer:YES];
        view = [[View alloc] initWithFrame:contentRect];
    }
    return self;
}

- (void) applicationWillFinishLaunching:(NSNotification *) notification {
    // Attach the view to the window
    [window setContentView:(id)view];
    [window setDelegate:view];
}

- (void) applicationDidFinishLaunching:(NSNotification *) notification {
    [self populateMainMenu];
    // Make the window visible
    [window makeKeyAndOrderFront:self];
}

- (void) dealloc {
    [view release];
    [window release];

    [super dealloc];
}

- (void) populateMainMenu {
    NSMenu *mainMenu = [[NSMenu alloc] initWithTitle:@"MainMenu"];
    NSMenuItem *menuItem;
    NSMenu *submenu;

    menuItem = [mainMenu addItemWithTitle:@"Apple" action:NULL keyEquivalent:@""];
    submenu = [[NSMenu alloc] initWithTitle:@"Apple"];
    [NSApp performSelector:NSSelectorFromString(@"setAppleMenu:") withObject:submenu];
    [self populateApplicationMenu:submenu];
    [mainMenu setSubmenu:submenu forItem:menuItem];

    [NSApp setMainMenu:mainMenu];
}

- (void) populateApplicationMenu:(NSMenu *) menu {
    NSString *applicationName = [[NSProcessInfo processInfo] processName];
    NSMenuItem *menuItem;

    menuItem = [menu
        addItemWithTitle:[NSString
            stringWithFormat:@"%@ %@",
            NSLocalizedString(@"About", nil),
            applicationName]
        action:@selector(orderFrontStandardAboutPanel:)
        keyEquivalent:@""];
    [menuItem setTarget:NSApp];

    [menu addItem:[NSMenuItem separatorItem]];

    menuItem = [menu
        addItemWithTitle:[NSString
        stringWithFormat:@"%@ %@",
        NSLocalizedString(@"Quit", nil), applicationName]
        action:@selector(terminate:)
        keyEquivalent:@"q"];
    [menuItem setTarget:NSApp];
}

@end

// Custom OpenGL View

@interface View : NSOpenGLView

{
    @public
    CVDisplayLinkRef displayLink;
    bool running;
    NSRecursiveLock* appLock;
}

- (void) drawView;

@end

@implementation View


- (id) initWithFrame:(NSRect) frame {
    running = true;

    // No multisampling
    int samples = 0;

    // Keep multisampling attributes at the start of the attribute lists since code below assumes they are array elements 0 through 4.
    NSOpenGLPixelFormatAttribute windowedAttrs[] =
    {
        NSOpenGLPFAMultisample,
        NSOpenGLPFASampleBuffers, samples ? 1 : 0,
        NSOpenGLPFASamples, samples,
        NSOpenGLPFAAccelerated,
        NSOpenGLPFADoubleBuffer,
        NSOpenGLPFAColorSize, 32,
        NSOpenGLPFADepthSize, 24,
        NSOpenGLPFAAlphaSize, 8,
        0
    };

    // Try to choose a supported pixel format
    NSOpenGLPixelFormat *pf = [[NSOpenGLPixelFormat alloc] initWithAttributes:windowedAttrs];

    if (!pf) {
        NSLog(@"OpenGL pixel format not supported.");
        return nil;
    }

    self = [super initWithFrame:frame pixelFormat:[pf autorelease]];
    appLock = [[NSRecursiveLock alloc] init];

    return self;
}

- (CVReturn) getFrameForTime:(const CVTimeStamp *) outputTime {
    [self drawView];
    return kCVReturnSuccess;
}


- (void) prepareOpenGL {
    [super prepareOpenGL];

    // Synchronize buffer swaps with vertical refresh rate
    GLint swapInt = 1; // Enable VSync
    [[self openGLContext] setValues:&swapInt forParameter:NSOpenGLCPSwapInterval];

    // Create a display link capable of being used with all active displays
    CVDisplayLinkCreateWithActiveCGDisplays(&displayLink);

    // Set the renderer output callback function
    CVDisplayLinkSetOutputCallback(displayLink, &GlobalDisplayLinkCallback, self);

    CGLContextObj cglContext = (CGLContextObj)[[self openGLContext] CGLContextObj];
    CGLPixelFormatObj cglPixelFormat = (CGLPixelFormatObj)[[self pixelFormat] CGLPixelFormatObj];
    CVDisplayLinkSetCurrentCGDisplayFromOpenGLContext(displayLink, cglContext, cglPixelFormat);

    [appLock lock];
    CGLLockContext((CGLContextObj)[[self openGLContext] CGLContextObj]);

    // Initialise client state (and OpenGL state)
    NSRect contentRect = [self frame];
    minecraftclient_client_initialise(
        &client_instance,
        (U32)contentRect.size.width,
        (U32)contentRect.size.height
    );
    init_next_external_event_ptr();

    CGLUnlockContext((CGLContextObj)[[self openGLContext] CGLContextObj]);
    [appLock unlock];

    // Activate the display link
    CVDisplayLinkStart(displayLink);
}

- (void) drawView {
    [appLock lock];
    CGLLockContext([[self openGLContext] CGLContextObj]);
    [[self openGLContext] makeCurrentContext];

    // Render
    next_external_event_ptr->event_type = PollNewEvents;
    minecraftclient_client_push_next_external_event(&client_instance);
    bool clientStillRunning = minecraftclient_client_process_events(&client_instance) != 0;

    CGLFlushDrawable([[self openGLContext] CGLContextObj]);
    CGLUnlockContext([[self openGLContext] CGLContextObj]);
    [appLock unlock];
}

- (void) reshape {
    printf("View::reshape\n");
    [super reshape];

    CGLLockContext([[self openGLContext] CGLContextObj]);

    // Report resize event to client.
    // Client will perform resize on next update.
    NSSize size = [self frame].size;
    if (next_external_event_ptr != NULL) {
        next_external_event_ptr->event_type = WindowEvent;
        next_external_event_ptr->event_data
            .window_event_data
            .event_type = ResizedWindowEvent;
        next_external_event_ptr->event_data
            .window_event_data
            .event_data
            .resized_data
            .width = (U32)size.width;
        next_external_event_ptr->event_data
            .window_event_data
            .event_data
            .resized_data
            .height = (U32)size.height;
        minecraftclient_client_push_next_external_event(&client_instance);
    }

    CGLUnlockContext([[self openGLContext] CGLContextObj]);
}

- (void) drawRect:(NSRect) dirtyRect {
    [self drawView];
}

- (BOOL) acceptsFirstResponder {
    return YES;
}

- (void) mouseDown:(NSEvent *) event {
    printf("View::mouseDown\n");
}

- (void) mouseDragged:(NSEvent *) event {
    printf("View::mouseDragged\n");
}

- (void) keyDown:(NSEvent *) event {
    if (next_external_event_ptr == NULL) return;
    // Send raw scancode to client.
    next_external_event_ptr->event_type = DeviceEvent;
    next_external_event_ptr->event_data
        .device_event_data
        .event_type = KeyDeviceEvent;
    next_external_event_ptr->event_data
        .device_event_data
        .event_data
        .key_data
        .mac_scancode = [event keyCode];
    next_external_event_ptr->event_data
        .device_event_data
        .event_data
        .key_data
        .element_state = PressedElementState;
    minecraftclient_client_push_next_external_event(&client_instance);
}

- (void) keyUp:(NSEvent *) event {
    if (next_external_event_ptr == NULL) return;
    // Send raw scancode to client.
    next_external_event_ptr->event_type = DeviceEvent;
    next_external_event_ptr->event_data
        .device_event_data
        .event_type = KeyDeviceEvent;
    next_external_event_ptr->event_data
        .device_event_data
        .event_data
        .key_data
        .mac_scancode = [event keyCode];
    next_external_event_ptr->event_data
        .device_event_data
        .event_data
        .key_data
        .element_state = ReleasedElementState;
    minecraftclient_client_push_next_external_event(&client_instance);
}

- (void) windowWillClose:(NSNotification *) notification {
    if (running) {
        running = false;

        [appLock lock];

        CVDisplayLinkStop(displayLink);
        CVDisplayLinkRelease(displayLink);

        [appLock unlock];
    }

    [NSApp terminate:self];
}

- (void) dealloc {
    [appLock release];
    [super dealloc];
}

@end

static CVReturn GlobalDisplayLinkCallback(
    CVDisplayLinkRef displayLink,
    const CVTimeStamp* now,
    const CVTimeStamp* outputTime,
    CVOptionFlags flagsIn,
    CVOptionFlags* flagsOut,
    void* displayLinkContext
) {
    CVReturn result = [(View*)displayLinkContext getFrameForTime:outputTime];
    return result;
}

void
trap(
    Trap trap
) {
    fprintf(stderr, "TRAP: %s\n", trapDescription(trap));
    abort();
}

extern char** environ;

int main(int argc, char *argv[]) {
    // Initialise the WASI and WASM environments
    if (!wasiInit(argc, argv, environ)) {
        fprintf(stderr, "failed to init WASI\n");
        return 1;
    }

    if (!wasiFileDescriptorAdd(-1, "/", NULL)) {
        fprintf(stderr, "failed to add preopen\n");
        return 1;
    }

    minecraftclientInstantiate(&client_instance, NULL);

    // Initialise app
    NSAutoreleasePool *pool = [[NSAutoreleasePool alloc] init];

    // Create a shared app instance.
    // This will initialize the global variable
    // 'NSApp' with the application instance.
    [NSApplication sharedApplication];
    AppDelegate *appDelegate = [[[AppDelegate alloc] init] autorelease];
    [NSApp setDelegate:appDelegate];
    [NSApp run];

    [pool drain];
    minecraftclientFreeInstance(&client_instance);
    return 0;
}
