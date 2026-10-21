use serde::{Deserialize, Serialize};

/// Aspect ratio and viewport layout helper for video display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoDimensions {
    pub width: u32,
    pub height: u32,
}

impl VideoDimensions {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Calculates the video aspect ratio (width / height).
    /// Defaults to 16:9 if height is 0.
    pub fn aspect_ratio(&self) -> f64 {
        if self.height == 0 {
            16.0 / 9.0
        } else {
            self.width as f64 / self.height as f64
        }
    }

    /// Fits the video dimensions inside a target viewport bounding box while
    /// preserving the aspect ratio. Returns `(fitted_width, fitted_height)`.
    ///
    /// - If viewport is wider than video -> pillarboxed (black bars on left/right).
    /// - If viewport is taller than video -> letterboxed (black bars on top/bottom).
    pub fn fit_in_viewport(&self, viewport_w: u32, viewport_h: u32) -> (u32, u32) {
        if self.width == 0 || self.height == 0 || viewport_w == 0 || viewport_h == 0 {
            return (viewport_w, viewport_h);
        }
        let video_ar = self.aspect_ratio();
        let viewport_ar = viewport_w as f64 / viewport_h as f64;

        if viewport_ar > video_ar {
            // Viewport is wider than video -> pillarboxed
            let fit_h = viewport_h;
            let fit_w = (viewport_h as f64 * video_ar).round() as u32;
            (fit_w, fit_h)
        } else {
            // Viewport is taller than video -> letterboxed
            let fit_w = viewport_w;
            let fit_h = (viewport_w as f64 / video_ar).round() as u32;
            (fit_w, fit_h)
        }
    }
}
