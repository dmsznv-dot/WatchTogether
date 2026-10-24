// Real GPU smoke test: create the surface, decode a fixture, read back pixels,
// resize, and destroy the renderer before the mpv handle. Requires brew mpv.
// With a second argument (expected fps, e.g. 60 for hd60.mp4) it also measures
// how many frames per second reach the screen during playback.
#import <AppKit/AppKit.h>
#import <OpenGL/gl3.h>
#include <mpv/client.h>
#include <mpv/render.h>
#include <stdio.h>
typedef struct {
    int (*create)(mpv_render_context **, mpv_handle *, mpv_render_param *);
    int (*render)(mpv_render_context *, mpv_render_param *);
    void (*free)(mpv_render_context *);
    void (*report_swap)(mpv_render_context *);
    uint64_t (*update)(mpv_render_context *);
    void (*set_update_callback)(mpv_render_context *, mpv_render_update_fn, void *);
} RenderApi;
static int swaps = 0;
static void counting_report_swap(mpv_render_context *ctx) {
    swaps++;
    mpv_render_context_report_swap(ctx);
}
extern void *wt_surface_create(void *, void *, RenderApi);
extern void wt_surface_destroy(void *);
static void pump(double seconds) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while ([deadline timeIntervalSinceNow] > 0)
        [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
}
static BOOL has_picture(NSOpenGLView *surface) {
    [surface display];
    [[surface openGLContext] makeCurrentContext];
    unsigned char pixels[64 * 64 * 4];
    glReadBuffer(GL_FRONT);
    glReadPixels(0, 0, 64, 64, GL_RGBA, GL_UNSIGNED_BYTE, pixels);
    unsigned char min = 255, max = 0;
    for (int i=0; i<64*64*4; i++) if (i % 4 != 3) {
        if (pixels[i] < min) min = pixels[i];
        if (pixels[i] > max) max = pixels[i];
    }
    printf("Framebuffer range: %u..%u\n", min, max);
    return max - min > 50;
}
int main(int argc, char **argv) {
    @autoreleasepool {
        if (argc != 2 && argc != 3) return 2;
        double expected_fps = argc == 3 ? atof(argv[2]) : 0;
        [NSApplication sharedApplication];
        NSRect frame = expected_fps > 0 ? NSMakeRect(0,0,960,540) : NSMakeRect(0,0,640,360);
        NSWindow *window = [[NSWindow alloc] initWithContentRect:frame
            styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
        [window setReleasedWhenClosed:NO];
        [window makeKeyAndOrderFront:nil];
        [NSApp finishLaunching];
        mpv_handle *mpv = mpv_create();
        mpv_set_option_string(mpv, "vo", "libmpv");
        mpv_set_option_string(mpv, "ao", "null");
        mpv_set_option_string(mpv, "hwdec", "auto-safe");
        if (mpv_initialize(mpv) < 0) return 3;
        RenderApi api = {mpv_render_context_create, mpv_render_context_render,
            mpv_render_context_free, counting_report_swap, mpv_render_context_update,
            mpv_render_context_set_update_callback};
        NSOpenGLView *surface = wt_surface_create(window.contentView, mpv, api);
        if (!surface) return 4;
        const char *args[] = {"loadfile", argv[1], NULL};
        if (mpv_command(mpv, args) < 0) return 5;
        pump(1.5);
        BOOL initial = has_picture(surface);
        BOOL smooth = YES;
        if (expected_fps > 0) {
            int before = swaps;
            pump(3.0);
            double fps = (swaps - before) / 3.0;
            char *drops = mpv_get_property_string(mpv, "frame-drop-count");
            char *delayed = mpv_get_property_string(mpv, "vo-delayed-frame-count");
            char *hwdec = mpv_get_property_string(mpv, "hwdec-current");
            printf("Presented %.1f fps (expected %.0f), decoder drops %s, delayed %s, hwdec %s\n",
                   fps, expected_fps, drops, delayed, hwdec);
            mpv_free(drops); mpv_free(delayed); mpv_free(hwdec);
            smooth = fps >= expected_fps * 0.9;
        }
        [window setContentSize:NSMakeSize(800,450)];
        pump(0.5);
        BOOL resized = has_picture(surface);
        wt_surface_destroy(surface);
        mpv_terminate_destroy(mpv);
        [window close];
        [window release];
        if (!initial || !resized) return 6;
        if (!smooth) return 7;
        puts("PASS: real libmpv frames render before and after resize; teardown succeeds");
    }
    return 0;
}
