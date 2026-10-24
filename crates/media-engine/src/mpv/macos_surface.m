// AppKit host for libmpv's stable OpenGL render API: a CAOpenGLLayer below the web view.
// No separate video window.
//
// HDR: while an HDR video (PQ/HLG) plays on a screen with extended dynamic range
// (XDR/HDR displays), the layer is EDR-enabled in extended linear sRGB and mpv outputs
// linear light: 1.0 = SDR white, brighter values up to the screen's current headroom.
// Otherwise frames are SDR and mpv tone-maps HDR video into that range.
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <OpenGL/OpenGL.h>
#import <OpenGL/gl3.h>
#include <dlfcn.h>
#include <math.h>
#include <stdatomic.h>

// Stable public ABI from mpv/client.h, mpv/render.h and mpv/render_gl.h.
typedef struct { int type; void *data; } WTRenderParam;
typedef struct { void *(*get_proc_address)(void *, const char *); void *ctx; } WTGLInit;
typedef struct { int fbo, w, h, internal_format; } WTFbo;
typedef struct {
    int (*create)(void **, void *, WTRenderParam *);
    int (*render)(void *, WTRenderParam *);
    void (*free)(void *);
    void (*report_swap)(void *);
    uint64_t (*update)(void *);
    void (*set_update_callback)(void *, void (*)(void *), void *);
    int (*set_property_string)(void *, const char *, const char *);
    char *(*get_property_string)(void *, const char *);
    void (*mpv_free)(void *);
} WTRenderApi;
static void *wt_gl_proc(void *ctx, const char *name) { (void)ctx; return dlsym(RTLD_DEFAULT, name); }

/// libmpv's linear light: 1.0 is SDR reference white at this luminance.
static const double WTSdrWhiteNits = 203.0;

@interface WTVideoLayer : CAOpenGLLayer {
@public
    void *renderer;
    void *mpv;
    WTRenderApi api;
    CGLContextObj cgl;
    CGLPixelFormatObj pixelFormat;
    BOOL floatBuffer;
    BOOL edr;
    double headroom;
    atomic_bool updatePending;
}
- (void)tick;
- (void)updateDynamicRangeForScreen:(NSScreen *)screen;
- (void)shutdown;
@end

// Called by mpv on its own thread whenever a new frame (or redraw) is due.
// Frames are presented as soon as they are ready instead of on a fixed timer,
// so 60 fps video does not drift against a 60 Hz poll and drop or repeat frames.
static void wt_on_update(void *ctx) {
    WTVideoLayer *layer = (WTVideoLayer *)ctx;
    if (atomic_exchange(&layer->updatePending, true)) return; // a tick is already queued
    dispatch_async(dispatch_get_main_queue(), ^{ [layer tick]; });
}

@implementation WTVideoLayer
- (CGLPixelFormatObj)copyCGLPixelFormatForDisplayMask:(uint32_t)mask {
    return CGLRetainPixelFormat(pixelFormat);
}
- (CGLContextObj)copyCGLContextForPixelFormat:(CGLPixelFormatObj)format {
    return CGLRetainContext(cgl);
}
- (BOOL)canDrawInCGLContext:(CGLContextObj)ctx pixelFormat:(CGLPixelFormatObj)format
               forLayerTime:(CFTimeInterval)t displayTime:(const CVTimeStamp *)ts {
    return renderer != NULL;
}
- (void)drawInCGLContext:(CGLContextObj)ctx pixelFormat:(CGLPixelFormatObj)format
            forLayerTime:(CFTimeInterval)t displayTime:(const CVTimeStamp *)ts {
    GLint fbo = 0, viewport[4] = {0, 0, 0, 0};
    glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &fbo);
    glGetIntegerv(GL_VIEWPORT, viewport);
    if (viewport[2] > 0 && viewport[3] > 0) {
        WTFbo target = {fbo, viewport[2], viewport[3], 0};
        int flip = 1, block = 0;
        // Do not block the UI thread waiting for the next presentation timestamp.
        WTRenderParam params[] = {{3, &target}, {4, &flip}, {12, &block}, {0, NULL}};
        api.render(renderer, params);
    }
    [super drawInCGLContext:ctx pixelFormat:format forLayerTime:t displayTime:ts];
    api.report_swap(renderer);
}
- (void)tick {
    atomic_store(&updatePending, false);
    if (!renderer) return;
    // Draw now, as mpv's own layer does: waiting for Core Animation to honour
    // setNeedsDisplay stalls mpv's frame queue and it starts dropping frames.
    if (api.update(renderer) & 1) [self display];
}
- (BOOL)playingHdrVideo {
    char *gamma = api.get_property_string(mpv, "video-params/gamma");
    BOOL hdr = gamma && (!strcmp(gamma, "pq") || !strcmp(gamma, "hlg"));
    if (gamma) api.mpv_free(gamma);
    return hdr;
}
- (void)updateDynamicRangeForScreen:(NSScreen *)screen {
    if (!renderer || !screen) return;
    BOOL want = floatBuffer && screen.maximumPotentialExtendedDynamicRangeColorComponentValue > 1.0
        && [self playingHdrVideo];
    // The current headroom follows display brightness (and rises once EDR is requested).
    double now = want ? fmax(1.0, screen.maximumExtendedDynamicRangeColorComponentValue) : 1.0;
    if (want == edr && fabs(now - headroom) < 0.05 * headroom) return;
    edr = want;
    headroom = now;
    self.wantsExtendedDynamicRangeContent = edr;
    CGColorSpaceRef space = CGColorSpaceCreateWithName(edr ? kCGColorSpaceExtendedLinearSRGB : kCGColorSpaceSRGB);
    self.colorspace = space;
    CGColorSpaceRelease(space);
    if (edr) {
        char peak[32];
        snprintf(peak, sizeof peak, "%.0f", WTSdrWhiteNits * headroom);
        api.set_property_string(mpv, "target-trc", "linear");
        api.set_property_string(mpv, "target-prim", "bt.709");
        api.set_property_string(mpv, "target-peak", peak);
    } else {
        api.set_property_string(mpv, "target-trc", "srgb");
        api.set_property_string(mpv, "target-prim", "bt.709");
        api.set_property_string(mpv, "target-peak", "auto");
    }
    [self setNeedsDisplay];
}
- (void)shutdown {
    if (renderer) {
        CGLSetCurrentContext(cgl);
        // No update callbacks run after free returns; queued ticks see renderer == NULL.
        api.set_update_callback(renderer, NULL, NULL);
        api.free(renderer);
        renderer = NULL;
    }
}
- (void)dealloc {
    if (cgl) CGLReleaseContext(cgl);
    if (pixelFormat) CGLReleasePixelFormat(pixelFormat);
    [super dealloc];
}
@end

// Layer-hosting view: passes mouse events to the web view above and keeps the layer's
// pixel density and dynamic range in step with the window's screen.
@interface WTVideoView : NSView {
@public
    WTVideoLayer *videoLayer;
    NSTimer *rangeTimer;
}
@end
@implementation WTVideoView
- (BOOL)isOpaque { return YES; }
- (BOOL)acceptsFirstResponder { return NO; }
- (NSView *)hitTest:(NSPoint)point { return nil; }
- (void)syncWithScreen {
    NSWindow *window = self.window;
    if (!window) return;
    videoLayer.contentsScale = window.backingScaleFactor;
    [videoLayer updateDynamicRangeForScreen:window.screen];
}
- (void)viewDidMoveToWindow { [super viewDidMoveToWindow]; [self syncWithScreen]; }
- (void)viewDidChangeBackingProperties { [super viewDidChangeBackingProperties]; [self syncWithScreen]; }
@end

static CGLPixelFormatObj wt_choose_format(BOOL wantFloat) {
    CGLPixelFormatAttribute attrs[16];
    int n = 0;
    attrs[n++] = kCGLPFAOpenGLProfile;
    attrs[n++] = (CGLPixelFormatAttribute)kCGLOGLPVersion_3_2_Core;
    attrs[n++] = kCGLPFAAccelerated;
    attrs[n++] = kCGLPFADoubleBuffer;
    attrs[n++] = kCGLPFAAllowOfflineRenderers;
    if (wantFloat) {
        // Half floats: values above 1.0 survive to the screen for EDR.
        attrs[n++] = kCGLPFAColorFloat;
        attrs[n++] = kCGLPFAColorSize;
        attrs[n++] = (CGLPixelFormatAttribute)64;
    } else {
        attrs[n++] = kCGLPFAColorSize;
        attrs[n++] = (CGLPixelFormatAttribute)24;
        attrs[n++] = kCGLPFAAlphaSize;
        attrs[n++] = (CGLPixelFormatAttribute)8;
    }
    attrs[n] = (CGLPixelFormatAttribute)0;
    CGLPixelFormatObj format = NULL;
    GLint count = 0;
    if (CGLChoosePixelFormat(attrs, &format, &count) != kCGLNoError) return NULL;
    return format;
}

void *wt_surface_create(void *parent_ptr, void *handle, WTRenderApi api) {
    NSCAssert([NSThread isMainThread], @"Video surface needs the main thread");
    NSView *parent = (NSView *)parent_ptr;
    BOOL floatBuffer = YES;
    CGLPixelFormatObj format = wt_choose_format(YES);
    if (!format) { floatBuffer = NO; format = wt_choose_format(NO); }
    if (!format) { NSLog(@"Cannot create an accelerated OpenGL pixel format"); return NULL; }
    CGLContextObj cgl = NULL;
    if (CGLCreateContext(format, NULL, &cgl) != kCGLNoError) {
        CGLReleasePixelFormat(format);
        NSLog(@"Cannot create an accelerated OpenGL context");
        return NULL;
    }
    CGLSetCurrentContext(cgl);
    void *renderer = NULL;
    WTGLInit init = {wt_gl_proc, NULL};
    WTRenderParam params[] = {{1, "opengl"}, {2, &init}, {0, NULL}};
    int result = api.create(&renderer, handle, params);
    if (result < 0) {
        NSLog(@"libmpv embedded OpenGL initialization failed: %d (GL: %s)", result, glGetString(GL_VERSION));
        CGLSetCurrentContext(NULL);
        CGLReleaseContext(cgl);
        CGLReleasePixelFormat(format);
        return NULL;
    }

    WTVideoLayer *layer = [[WTVideoLayer alloc] init];
    layer->api = api;
    layer->mpv = handle;
    layer->renderer = renderer;
    layer->cgl = cgl;
    layer->pixelFormat = format;
    layer->floatBuffer = floatBuffer;
    layer->headroom = 1.0;
    atomic_init(&layer->updatePending, false);
    layer.asynchronous = NO;
    layer.opaque = YES;
    layer.needsDisplayOnBoundsChange = YES;
    layer.backgroundColor = CGColorGetConstantColor(kCGColorBlack);

    WTVideoView *view = [[WTVideoView alloc] initWithFrame:parent.bounds];
    view->videoLayer = layer;
    view.layer = layer; // layer-hosting: set before wantsLayer
    view.wantsLayer = YES;
    view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    [parent addSubview:view positioned:NSWindowBelow relativeTo:nil];
    [view syncWithScreen];
    // HDR content starts and stops with files; headroom changes with brightness.
    view->rangeTimer = [NSTimer timerWithTimeInterval:1.0 target:view selector:@selector(syncWithScreen)
                                             userInfo:nil repeats:YES];
    [[NSRunLoop mainRunLoop] addTimer:view->rangeTimer forMode:NSRunLoopCommonModes];
    api.set_update_callback(renderer, wt_on_update, layer);
    return view;
}

void wt_surface_destroy(void *ptr) {
    NSCAssert([NSThread isMainThread], @"Video teardown needs the main thread");
    WTVideoView *view = (WTVideoView *)ptr;
    [view->rangeTimer invalidate];
    view->rangeTimer = nil;
    [view->videoLayer shutdown];
    [view removeFromSuperview];
    [view->videoLayer release];
    [view release];
}
