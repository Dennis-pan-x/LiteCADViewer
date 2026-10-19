#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, scale: 1.0 }
    }
}

impl Camera {
    pub fn world(&self, dx: f64, dy: f64) -> (f64, f64) {
        (self.x + dx / self.scale, self.y - dy / self.scale)
    }
    pub fn pan(&mut self, dx: f64, dy: f64) {
        if dx.is_finite() && dy.is_finite() {
            self.x -= dx / self.scale;
            self.y += dy / self.scale;
        }
    }
    pub fn zoom(&mut self, factor: f64, dx: f64, dy: f64) {
        if !factor.is_finite() || factor <= 0.0 || !dx.is_finite() || !dy.is_finite() {
            return;
        }
        let anchor = self.world(dx, dy);
        self.scale = (self.scale * factor).clamp(1e-9, 1e10);
        self.x = anchor.0 - dx / self.scale;
        self.y = anchor.1 + dy / self.scale;
    }
}

/// Uniform render density keeps a large/high-DPI canvas fully covered at a
/// bounded raster size; independently clamping width and height would crop it.
pub fn raster_size(width: f32, height: f32, ppp: f32) -> (u32, u32, f32) {
    let width = if width.is_finite() { width.max(1.0) } else { 1.0 };
    let height = if height.is_finite() { height.max(1.0) } else { 1.0 };
    let ppp = if ppp.is_finite() { ppp.max(0.01) } else { 1.0 };
    let density = ppp.min(4096.0 / width).min(4096.0 / height);
    ((width * density).round().clamp(1.0, 4096.0) as u32, (height * density).round().clamp(1.0, 4096.0) as u32, density)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_the_drawing_point_under_the_cursor() {
        let mut c = Camera { x: 1e9, y: -2e9, scale: 3.0 };
        let before = c.world(150.0, -70.0);
        c.zoom(1.4, 150.0, -70.0);
        let after = c.world(150.0, -70.0);
        assert!((before.0 - after.0).abs() < 1e-6);
        assert!((before.1 - after.1).abs() < 1e-6);
    }

    #[test]
    fn pan_follows_the_pointer_and_rejects_invalid_zoom() {
        let mut c = Camera { x: 0.0, y: 0.0, scale: 2.0 };
        c.pan(30.0, -20.0);
        assert_eq!((c.x, c.y), (-15.0, -10.0));
        let before = c;
        c.zoom(f64::NAN, 0.0, 0.0);
        assert_eq!(c, before);
    }

    #[test]
    fn high_dpi_raster_covers_the_entire_canvas() {
        let (w, h, density) = raster_size(6000.0, 3000.0, 2.0);
        assert_eq!((w, h), (4096, 2048));
        assert!((w as f32 / density - 6000.0).abs() < 0.01);
        assert!((h as f32 / density - 3000.0).abs() < 0.01);
    }
}
