//! Viewer GPU meshes and their view-independent cache. The backend is shared with the editor.
use crate::document::Document;
use cadcraft_geom::Vec2;
use cadcraft_render::raster::View;
use cadcraft_render_wgpu as gpu;
use egui_wgpu::wgpu;

pub struct Frame {
    pub data: gpu::MeshData,
    pub cache: Cache,
}

pub struct Cache {
    pub origin: Vec2,
    pub tolerance: f64,
    pub dark: bool,
}

impl Cache {
    pub fn serves(&self, view: View, dark: bool) -> bool {
        let offset_pixels = (self.origin.x - view.center.x).abs().max((self.origin.y - view.center.y).abs()) * view.scale;
        // Keep the existing origin while f32 position/uniform rounding stays below
        // a quarter pixel. A continuously changing origin grid defeats deep-zoom caching.
        self.dark == dark && self.tolerance * view.scale <= 0.5 && offset_pixels <= 1_000_000.0
    }
}

pub fn build(d: &mut Document, view: View, dark: bool, device: Option<&wgpu::Device>) -> Result<Frame, String> {
    d.refine_for_view(view)?;
    let origin = gpu::choose_origin(&d.list.bounds, view.center, f64::from(view.height) / view.scale);
    let color = if d.space.is_paper() || !dark { crate::theme::PAPER } else { crate::theme::CANVAS };
    let bg = cadcraft_color::Rgb(color.r(), color.g(), color.b());
    let mut data = gpu::build_ordered_mesh(&d.list, origin, bg, false);
    d.check_cancelled()?;
    if let Some(device) = device {
        data.upload_now(device);
    }
    d.check_cancelled()?;
    Ok(Frame { data, cache: Cache { origin, tolerance: d.tolerance, dark } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_geom::Bounds2;
    #[test]
    fn deep_zoom_keeps_a_precise_existing_origin_across_small_zoom_steps() {
        let bounds = Bounds2::new(Vec2::ZERO, Vec2::new(1e6, 1e6));
        let mut view = View { center: Vec2::new(123456.7, 654321.2), scale: 100.0, width: 1600, height: 1000 };
        let cache = Cache { origin: gpu::choose_origin(&bounds, view.center, 10.0), tolerance: 0.001, dark: true };
        for _ in 0..30 {
            view.scale *= 1.002;
            assert!(cache.serves(view, true), "small zoom steps must not re-upload a precise mesh");
        }
        view.center.x += 1e6;
        assert!(!cache.serves(view, true), "rebase once float precision would be lost");
    }
    #[test]
    fn filled_geometry_after_a_line_preserves_primitive_order() {
        use cadcraft_doc::{Common, Drawing, EntityKind, Line, Solid, Space};
        use cadcraft_geom::Vec3;
        let dir = tempfile::tempdir().expect("fresh directory");
        let path = dir.path().join("ordered.dxf");
        let mut drawing = Drawing::new_metric();
        drawing.add(&Space::Model, Common::default(), EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::new(100.0, 100.0, 0.0) })).expect("line");
        drawing
            .add(
                &Space::Model,
                Common::default(),
                EntityKind::Solid(Solid {
                    corners: [Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 0.0), Vec3::new(0.0, 100.0, 0.0)],
                }),
            )
            .expect("fill");
        std::fs::write(&path, cadcraft_io::write(&drawing, "order.dxf").expect("DXF")).expect("input");
        let mut doc = Document::open(&path).expect("read");
        let view = View::fit(&doc.list.bounds, 800, 600, 0.08);
        let frame = build(&mut doc, view, true, None).expect("GPU mesh");
        assert!(!frame.data.breaks.is_empty(), "a later fill must draw after earlier lines");
    }
    #[test]
    fn mesh_preserves_infinite_lines_and_can_fall_back_to_cpu_without_writing_source() {
        use cadcraft_doc::{Common, Drawing, EntityKind, Line, RayLine, Space};
        use cadcraft_geom::Vec3;
        let dir = tempfile::tempdir().expect("fresh input folder");
        let path = dir.path().join("infinite.dxf");
        let mut drawing = Drawing::new_metric();
        drawing.add(&Space::Model, Common::default(), EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::new(100.0, 100.0, 0.0) })).expect("line");
        drawing
            .add(&Space::Model, Common::default(), EntityKind::XLine(RayLine { base: Vec3::ZERO, dir: Vec3::new(1.0, 0.0, 0.0) }))
            .expect("infinite line");
        let bytes = cadcraft_io::write(&drawing, "infinite.dxf").expect("DXF");
        std::fs::write(&path, &bytes).expect("new input");
        let mut doc = Document::open(&path).expect("read");
        let view = View::fit(&doc.list.bounds, 800, 600, 0.08);
        let frame = build(&mut doc, view, true, None).expect("mesh without a device");
        assert!(frame.data.line_vertices() > 0);
        assert_eq!(frame.data.quad_instances(), 1);
        assert!(frame.cache.serves(view, true));
        assert!(!doc.render(view, true).expect("CPU fallback").is_empty());
        assert_eq!(std::fs::read(&path).expect("input after"), bytes);
    }
    #[test]
    fn camera_changes_reuse_mesh_but_precision_and_background_changes_do_not() {
        let bounds = Bounds2::new(Vec2::ZERO, Vec2::new(1000.0, 1000.0));
        let cache = Cache { origin: bounds.center(), tolerance: 0.1, dark: true };
        let mut view = View::fit(&bounds, 1600, 1000, 0.08);
        assert!(cache.serves(view, true));
        view.center.x += 100.0;
        view.scale *= 1.2;
        assert!(cache.serves(view, true), "ordinary pan/zoom only updates uniforms");
        assert!(!cache.serves(view, false));
        view.scale = 100.0;
        assert!(!cache.serves(view, true), "deep zoom requires finer curves");
    }
}
