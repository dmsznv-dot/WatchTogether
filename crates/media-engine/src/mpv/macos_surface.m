// AppKit host for libmpv's stable OpenGL render API. No separate video window.
#import <AppKit/AppKit.h>
#import <OpenGL/gl3.h>
#include <dlfcn.h>
#include <stdatomic.h>

// Stable public ABI from mpv/render.h and mpv/render_gl.h.
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
} WTRenderApi;
static void *wt_gl_proc(void *ctx, const char *name) { (void)ctx; return dlsym(RTLD_DEFAULT, name); }

@interface WTVideoSurface : NSOpenGLView {
@public
    void *renderer;
    WTRenderApi api;
    atomic_bool updatePending;
}
- (void)tick;
- (void)renderFrame;
- (void)shutdown;
@end

// Called by mpv on its own thread whenever a new frame (or redraw) is due.
// Frames are presented as soon as they are ready instead of on a fixed timer,
// so 60 fps video does not drift against a 60 Hz poll and drop or repeat frames.
static void wt_on_update(void *ctx) {
    WTVideoSurface *surface = (WTVideoSurface *)ctx;
    if (atomic_exchange(&surface->updatePending, true)) return; // a tick is already queued
    dispatch_async(dispatch_get_main_queue(), ^{ [surface tick]; });
}
@implementation WTVideoSurface
- (BOOL)isOpaque { return YES; }
- (BOOL)acceptsFirstResponder { return NO; }
- (NSView *)hitTest:(NSPoint)point { return nil; }
- (void)tick {
    atomic_store(&updatePending, false);
    if (!renderer) return;
    [[self openGLContext] makeCurrentContext];
    if (api.update(renderer) & 1) [self renderFrame];
}
- (void)reshape {
    [super reshape];
    [[self openGLContext] update];
}
- (void)drawRect:(NSRect)dirtyRect {
    [self renderFrame];
}
- (void)renderFrame {
    if (!renderer) return;
    [[self openGLContext] makeCurrentContext];
    NSRect pixels = [self convertRectToBacking:self.bounds];
    if (pixels.size.width < 1 || pixels.size.height < 1) return;
    WTFbo fbo = {0, (int)pixels.size.width, (int)pixels.size.height, 0};
    int flip = 1, block = 0;
    // Do not block the UI thread waiting for the next presentation timestamp.
    WTRenderParam params[] = {{3, &fbo}, {4, &flip}, {12, &block}, {0, NULL}};
    api.render(renderer, params);
    [[self openGLContext] flushBuffer];
    api.report_swap(renderer);
}
- (void)shutdown {
    if (renderer) {
        [[self openGLContext] makeCurrentContext];
        // No update callbacks run after free returns; queued ticks see renderer == NULL.
        api.set_update_callback(renderer, NULL, NULL);
        api.free(renderer);
        renderer = NULL;
    }
}
@end

void *wt_surface_create(void *parent_ptr, void *handle, WTRenderApi api) {
    NSCAssert([NSThread isMainThread], @"Video surface needs the main thread");
    NSView *parent = (NSView *)parent_ptr;
    NSOpenGLPixelFormatAttribute attrs[] = {
        NSOpenGLPFAOpenGLProfile, NSOpenGLProfileVersion3_2Core,
        NSOpenGLPFAAccelerated, NSOpenGLPFADoubleBuffer,
        NSOpenGLPFAColorSize, 24, NSOpenGLPFAAlphaSize, 8, 0
    };
    NSOpenGLPixelFormat *format = [[NSOpenGLPixelFormat alloc] initWithAttributes:attrs];
    if (!format) { NSLog(@"Cannot create an accelerated OpenGL pixel format"); return NULL; }
    WTVideoSurface *surface = [[WTVideoSurface alloc] initWithFrame:parent.bounds pixelFormat:format];
    [format release];
    if (!surface) return NULL;
    surface->api = api;
    surface.wantsBestResolutionOpenGLSurface = YES;
    surface.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    [parent addSubview:surface positioned:NSWindowBelow relativeTo:nil];
    [[surface openGLContext] makeCurrentContext];
    GLint interval = 1;
    [[surface openGLContext] setValues:&interval forParameter:NSOpenGLContextParameterSwapInterval];
    WTGLInit init = {wt_gl_proc, NULL};
    WTRenderParam params[] = {{1, "opengl"}, {2, &init}, {0, NULL}};
    int result = api.create(&surface->renderer, handle, params);
    if (result < 0) {
        NSLog(@"libmpv embedded OpenGL initialization failed: %d (GL: %s)", result, glGetString(GL_VERSION));
        [surface removeFromSuperview];
        [surface release];
        return NULL;
    }
    atomic_init(&surface->updatePending, false);
    api.set_update_callback(surface->renderer, wt_on_update, surface);
    return surface;
}
void wt_surface_destroy(void *ptr) {
    NSCAssert([NSThread isMainThread], @"Video teardown needs the main thread");
    WTVideoSurface *surface = (WTVideoSurface *)ptr;
    [surface shutdown];
    [surface removeFromSuperview];
    [surface release];
}
