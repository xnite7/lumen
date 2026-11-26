// The film on a Mac: mpv draws through its render API into an OpenGL layer
// (the way IINA does it), in a view placed just under the app's own view.
// The app's view is see-through where there's no UI, so the film shows
// through it. (mpv's --wid embedding isn't reliable on macOS: black video.)
#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>
#import <OpenGL/gl3.h>
#include <mpv/client.h>
#include <mpv/render_gl.h>

static void *couch_gl_proc(void *ctx, const char *name) {
    (void)ctx;
    CFStringRef s = CFStringCreateWithCString(kCFAllocatorDefault, name, kCFStringEncodingASCII);
    void *addr = CFBundleGetFunctionPointerForName(CFBundleGetBundleWithIdentifier(CFSTR("com.apple.opengl")), s);
    CFRelease(s);
    return addr;
}

@interface CouchVideoLayer : CAOpenGLLayer
@property(atomic) mpv_handle *mpv;
@property(atomic) mpv_render_context *gl;
@end

static void couch_gl_update(void *ctx) {
    CouchVideoLayer *layer = (__bridge CouchVideoLayer *)ctx;
    dispatch_async(dispatch_get_main_queue(), ^{
        [layer setNeedsDisplay];
    });
}

@implementation CouchVideoLayer
- (instancetype)init {
    self = [super init];
    if (self) {
        self.asynchronous = NO;
        self.opaque = YES;
        self.needsDisplayOnBoundsChange = YES;
        self.backgroundColor = CGColorGetConstantColor(kCGColorBlack);
        self.autoresizingMask = kCALayerWidthSizable | kCALayerHeightSizable;
    }
    return self;
}

- (CGLPixelFormatObj)copyCGLPixelFormatForDisplayMask:(uint32_t)mask {
    (void)mask;
    CGLPixelFormatAttribute attrs[] = {
        kCGLPFAOpenGLProfile, (CGLPixelFormatAttribute)kCGLOGLPVersion_3_2_Core,
        kCGLPFADoubleBuffer, kCGLPFAAllowOfflineRenderers, kCGLPFAAccelerated,
        kCGLPFASupportsAutomaticGraphicsSwitching, (CGLPixelFormatAttribute)0};
    CGLPixelFormatObj pix = NULL;
    GLint n = 0;
    CGLChoosePixelFormat(attrs, &pix, &n);
    if (!pix) {
        // no accelerated 3.2 context: whatever there is
        CGLPixelFormatAttribute basic[] = {kCGLPFADoubleBuffer, (CGLPixelFormatAttribute)0};
        CGLChoosePixelFormat(basic, &pix, &n);
    }
    return pix;
}

- (CGLContextObj)copyCGLContextForPixelFormat:(CGLPixelFormatObj)pf {
    CGLContextObj ctx = NULL;
    CGLCreateContext(pf, NULL, &ctx);
    GLint one = 1;
    CGLSetParameter(ctx, kCGLCPSwapInterval, &one);
    return ctx;
}

- (BOOL)canDrawInCGLContext:(CGLContextObj)ctx pixelFormat:(CGLPixelFormatObj)pf forLayerTime:(CFTimeInterval)t displayTime:(const CVTimeStamp *)ts {
    return YES;
}

- (void)drawInCGLContext:(CGLContextObj)ctx pixelFormat:(CGLPixelFormatObj)pf forLayerTime:(CFTimeInterval)t displayTime:(const CVTimeStamp *)ts {
    CGLSetCurrentContext(ctx);
    // the render context is made the first time there's a GL context and an mpv
    if (!self.gl && self.mpv) {
        mpv_opengl_init_params gl_init = {.get_proc_address = couch_gl_proc};
        int adv = 1;
        mpv_render_param params[] = {
            {MPV_RENDER_PARAM_API_TYPE, (void *)MPV_RENDER_API_TYPE_OPENGL},
            {MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, &gl_init},
            {MPV_RENDER_PARAM_ADVANCED_CONTROL, &adv},
            {0}};
        mpv_render_context *gl = NULL;
        if (mpv_render_context_create(&gl, self.mpv, params) >= 0) {
            self.gl = gl;
            mpv_render_context_set_update_callback(gl, couch_gl_update, (__bridge void *)self);
        }
    }
    GLint fbo = 0, dims[4] = {0, 0, 0, 0};
    glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &fbo);
    glGetIntegerv(GL_VIEWPORT, dims);
    if (self.gl && dims[2] > 0 && dims[3] > 0) {
        mpv_opengl_fbo f = {.fbo = fbo, .w = dims[2], .h = dims[3], .internal_format = 0};
        int flip = 1;
        mpv_render_param p[] = {{MPV_RENDER_PARAM_OPENGL_FBO, &f}, {MPV_RENDER_PARAM_FLIP_Y, &flip}, {0}};
        mpv_render_context_render(self.gl, p);
    } else {
        glClearColor(0, 0, 0, 1);
        glClear(GL_COLOR_BUFFER_BIT);
    }
    [super drawInCGLContext:ctx pixelFormat:pf forLayerTime:t displayTime:ts];
    if (self.gl)
        mpv_render_context_report_swap(self.gl);
}
@end

// the film's view never takes the mouse: clicks go to the app's view
@interface CouchVideoView : NSView
@end
@implementation CouchVideoView
- (NSView *)hitTest:(NSPoint)p { (void)p; return nil; }
- (BOOL)acceptsFirstResponder { return NO; }
@end

static NSView *g_ui;          // the app's own view (GPUI)
static NSView *g_video;       // the film's view, just under it
static CouchVideoLayer *g_layer;

// Called once the window exists, with the app's view.
void couch_mac_video_attach(void *ui_view) {
    void (^go)(void) = ^{
        if (g_video || !ui_view)
            return;
        g_ui = (__bridge NSView *)ui_view;
        NSView *host = g_ui.superview;
        if (!host)
            return;
        g_layer = [CouchVideoLayer layer];
        g_layer.contentsScale = g_ui.window.backingScaleFactor ?: 2.0;
        g_video = [[CouchVideoView alloc] initWithFrame:g_ui.frame];
        g_video.layer = g_layer;      // layer-hosting
        g_video.wantsLayer = YES;
        g_video.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        g_video.hidden = YES;          // until something plays
        [host addSubview:g_video positioned:NSWindowBelow relativeTo:g_ui];
        // the app's view draws see-through where it has nothing to draw
        g_ui.window.opaque = NO;
        g_ui.window.backgroundColor = NSColor.blackColor;
    };
    if (NSThread.isMainThread) go(); else dispatch_sync(dispatch_get_main_queue(), go);
}

// mpv is ready (made with vo=libmpv): the layer draws it from now on.
void couch_mac_video_set_mpv(void *mpv) {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (!g_layer)
            return;
        g_layer.mpv = (mpv_handle *)mpv;
        [g_layer setNeedsDisplay];
    });
}

// Where the film goes, in the app's view coordinates (top-left origin, points).
// w <= 0: the whole window. radius: rounded corners (the mini player).
void couch_mac_video_place(double x, double y, double w, double h, double radius, int visible) {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (!g_video || !g_ui)
            return;
        g_video.hidden = !visible;
        NSRect ui = g_ui.frame;
        NSRect r = ui;
        if (w > 0 && h > 0) {
            CGFloat top = g_ui.isFlipped ? y : ui.size.height - y - h;
            r = NSMakeRect(ui.origin.x + x, ui.origin.y + top, w, h);
            g_video.autoresizingMask = NSViewNotSizable;
        } else {
            g_video.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        }
        g_video.frame = r;
        // the corner: over the app's view (the page scrolls under it, like the
        // Windows mini player's own window); full size: under it, the controls on top
        NSView *host = g_ui.superview;
        BOOL corner = w > 0 && h > 0;
        [g_video removeFromSuperview];
        [host addSubview:g_video positioned:(corner ? NSWindowAbove : NSWindowBelow) relativeTo:g_ui];
        g_layer.contentsScale = g_ui.window.backingScaleFactor ?: 2.0;
        g_layer.cornerRadius = radius;
        g_layer.masksToBounds = radius > 0;
        // the mini player: only the top corners round (its bar sits under it)
        if (@available(macOS 10.13, *))
            g_layer.maskedCorners = kCALayerMinXMaxYCorner | kCALayerMaxXMaxYCorner;
        [g_layer setNeedsDisplay];
    });
}
