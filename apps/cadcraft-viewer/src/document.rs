use cadcraft_color::Rgb;
use cadcraft_doc::{Drawing, EntityKind, Space};
use cadcraft_render::raster::{RasterOptions, View};
use cadcraft_render::{DisplayList, Options};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Default)]
pub struct OpenTimings {
    pub read_ms: f64,
    pub parse_ms: f64,
    pub list_ms: f64,
}

pub struct Document {
    pub source: PathBuf,
    pub drawing: Drawing,
    pub space: Space,
    pub list: DisplayList,
    pub notes: Vec<String>,
    pub tolerance: f64,
    pub timings: OpenTimings,
    cancel: Arc<AtomicBool>,
}

impl Document {
    pub fn use_cancel(&mut self, cancel: Arc<AtomicBool>) {
        self.cancel = cancel;
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        Self::open_with_cancel(path, Arc::new(AtomicBool::new(false)))
    }
    pub fn open_with_cancel(path: &Path, cancel: Arc<AtomicBool>) -> Result<Self, String> {
        let read_start = std::time::Instant::now();
        if cancel.load(Ordering::Relaxed) {
            return Err("Superseded request.".into());
        }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if !matches!(ext.as_str(), "dwg" | "dxf" | "dwt") {
            return Err("Only DWG, DXF and DWT drawings are supported.".into());
        }
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
            return Err("Not a regular file, or larger than the 256 MiB opening limit.".into());
        }
        use std::io::Read;
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("File grew beyond the opening limit.".into());
        }
        let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
        let parse_start = std::time::Instant::now();
        let drawing = cadcraft_io::read(&bytes, &path.to_string_lossy()).map_err(|e| e.to_string())?;
        let parse_ms = parse_start.elapsed().as_secs_f64() * 1000.0;
        if cancel.load(Ordering::Relaxed) {
            return Err("Superseded request.".into());
        }
        let mut notes = drawing.read_notes.clone();
        if !drawing.skipped_records.is_empty() {
            notes.push(format!("Skipped input records: {:?}", drawing.skipped_records));
        }
        let unknown = drawing
            .model
            .iter()
            .chain(drawing.layouts.iter().flat_map(|l| l.entities.iter()))
            .chain(drawing.blocks.values().flat_map(|b| b.entities.iter()))
            .filter(|e| matches!(e.kind, EntityKind::Unknown(_)))
            .count();
        if unknown > 0 {
            notes.push(format!("{unknown} unsupported/proxy objects: their appearance is not guaranteed."));
        }
        let shx: Vec<_> = drawing.text_styles.iter().filter(|s| s.font.to_ascii_lowercase().ends_with(".shx")).map(|s| s.font.clone()).collect();
        if !shx.is_empty() {
            notes.push(format!("SHX fonts use fallback shapes; text may differ: {}", shx.join(", ")));
        }
        let list_start = std::time::Instant::now();
        let mut result = Self {
            source: path.to_owned(),
            drawing,
            space: Space::Model,
            list: DisplayList::default(),
            notes,
            tolerance: 0.01,
            timings: OpenTimings { read_ms, parse_ms, list_ms: 0.0 },
            cancel,
        };
        result.reset_tolerance();
        result.rebuild()?;
        result.timings.list_ms = list_start.elapsed().as_secs_f64() * 1000.0;
        Ok(result)
    }
    fn reset_tolerance(&mut self) {
        let bounds = self.drawing.extents(&self.space);
        let span = bounds.width().max(bounds.height());
        // At most half a pixel over a 4096-pixel fitted viewport. Deep zoom refines later.
        self.tolerance = if !bounds.is_empty() && span.is_finite() && span > 0.0 { (span / 8192.0).max(1e-10) } else { 0.01 };
    }
    pub fn check_cancelled(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) { Err("Superseded request.".into()) } else { Ok(()) }
    }
    pub fn rebuild(&mut self) -> Result<(), String> {
        let list = cadcraft_render::build(
            &self.drawing,
            &self.space,
            &Options { tolerance: self.tolerance, transparency: true, cancel: Some(self.cancel.clone()), ..Default::default() },
        );
        if self.cancel.load(Ordering::Relaxed) {
            return Err("Superseded request.".into());
        }
        self.list = list;
        Ok(())
    }
    pub fn set_layer(&mut self, name: &str, on: bool) -> Result<(), String> {
        self.set_layers(&[(name.to_owned(), on)])
    }
    pub fn set_layers(&mut self, changes: &[(String, bool)]) -> Result<(), String> {
        self.apply_layers(changes);
        self.rebuild()
    }
    /// State changes must survive cancellation of their expensive visual rebuild.
    pub fn apply_layers(&mut self, changes: &[(String, bool)]) {
        for (name, on) in changes {
            if let Some(l) = self.drawing.layer_mut(name) {
                l.on = *on;
                l.frozen = !*on;
            }
        }
    }
    pub fn set_space(&mut self, name: &str) -> Result<(), String> {
        self.space = if name.is_empty() {
            Space::Model
        } else if self.drawing.layout(name).is_some() {
            Space::Paper(name.to_owned())
        } else {
            return Err("Layout does not exist.".into());
        };
        self.reset_tolerance();
        self.rebuild()?;
        Ok(())
    }
    pub fn refine_for_view(&mut self, view: View) -> Result<(), String> {
        self.check_cancelled()?;
        if !view.scale.is_finite() || view.scale <= 0.0 || !view.center.x.is_finite() || !view.center.y.is_finite() {
            return Err("Invalid view.".into());
        }
        // Refine curved geometry only when the cached chord error exceeds 1/2 px.
        if self.tolerance * view.scale > 0.5 {
            let previous = self.tolerance;
            self.tolerance = (0.25 / view.scale).max(1e-10);
            if let Err(e) = self.rebuild() {
                self.tolerance = previous;
                return Err(e);
            }
        }
        Ok(())
    }
    pub fn render(&mut self, view: View, dark: bool) -> Result<Vec<u8>, String> {
        self.refine_for_view(view)?;
        let color = if self.space.is_paper() || !dark { crate::theme::PAPER } else { crate::theme::CANVAS };
        let background = Rgb(color.r(), color.g(), color.b());
        let image = cadcraft_render::raster::render(&self.list, &view, &RasterOptions { background, ..Default::default() })
            .ok_or("Could not allocate the drawing image.")?;
        if self.cancel.load(Ordering::Relaxed) {
            return Err("Superseded request.".into());
        }
        // tiny-skia uses premultiplied RGBA; egui's matching upload path preserves edges.
        Ok(image.data().to_vec())
    }
    pub fn export_pdf(&self, output: &Path) -> Result<(), String> {
        if output.extension().and_then(|s| s.to_str()).is_none_or(|s| !s.eq_ignore_ascii_case("pdf")) {
            return Err("Choose a .pdf output file.".into());
        }
        if output == self.source || std::fs::canonicalize(output).ok().zip(std::fs::canonicalize(&self.source).ok()).is_some_and(|(a, b)| a == b) {
            return Err("The source drawing cannot be overwritten.".into());
        }
        let pdf = cadcraft_io::pdf::pdf(&self.drawing, &self.space, &cadcraft_io::pdf::PdfOptions { compress: true, ..Default::default() })
            .map_err(|e| e.to_string())?;
        write_new(output, &pdf)
    }
}

/// Never follow or truncate an existing output entry (including source hard links).
/// Export deliberately refuses replacement; the user can choose another output name.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| e.to_string())?;
    file.write_all(bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::{Common, EntityKind, Layout, Line};
    use cadcraft_geom::Vec3;

    #[test]
    fn large_radius_opening_uses_screen_bounded_curves_and_refines_on_zoom() {
        let dir = tempfile::tempdir().expect("fresh directory");
        let path = dir.path().join("large-circle.dxf");
        let mut drawing = Drawing::new_metric();
        drawing
            .add(&Space::Model, Common::default(), EntityKind::Circle(cadcraft_doc::Circle { center: Vec3::ZERO, radius: 1_000_000.0 }))
            .expect("circle");
        std::fs::write(&path, cadcraft_io::write(&drawing, "circle.dxf").expect("DXF")).expect("input");
        let mut d = Document::open(&path).expect("open");
        let initial = d.list.verts.len();
        assert!(initial < 1024, "opening over-tessellates: {initial} vertices");
        let before = d.tolerance;
        d.render(View { center: cadcraft_geom::Vec2::ZERO, scale: 0.1, width: 800, height: 600 }, true).expect("zoom");
        assert!(d.tolerance < before, "zoom refines the initial approximation");
        assert!(d.tolerance * 0.1 <= 0.5);
    }

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temporary test folder");
        let path = dir.path().join("fresh.dxf");
        let mut drawing = Drawing::new_metric();
        drawing
            .add(
                &Space::Model,
                Common { layer: "DETAIL".into(), ..Default::default() },
                EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::new(100.0, 40.0, 0.0) }),
            )
            .expect("model line");
        drawing.layouts.push(Layout::new("Far layout", 1));
        drawing
            .add(
                &Space::Paper("Far layout".into()),
                Common::default(),
                EntityKind::Line(Line { a: Vec3::new(10000.0, 20000.0, 0.0), b: Vec3::new(10500.0, 20200.0, 0.0) }),
            )
            .expect("layout line");
        let bytes = cadcraft_io::write(&drawing, "fresh.dxf").expect("synthetic DXF");
        std::fs::write(&path, bytes).expect("test input");
        (dir, path)
    }
    #[test]
    fn layer_and_layout_navigation_leave_source_bytes_unchanged() {
        let (_dir, path) = fixture();
        let source = std::fs::read(&path).expect("input");
        let mut d = Document::open(&path).expect("open");
        let before = d.list.prims.len();
        d.set_layer("DETAIL", false).expect("hide layer");
        assert!(d.list.prims.len() < before);
        d.set_layer("DETAIL", true).expect("show layer");
        assert_eq!(d.list.prims.len(), before);
        let model = d.list.bounds;
        d.set_space("Far layout").expect("layout");
        assert_ne!(d.list.bounds, model);
        let view = View::fit(&d.list.bounds, 800, 600, 0.1);
        assert!(view.center.y > 10000.0);
        d.render(view, true).expect("render");
        assert_eq!(std::fs::read(path).expect("input after"), source);
    }
    #[test]
    fn existing_output_and_source_hard_links_are_never_truncated() {
        let (dir, path) = fixture();
        let source = std::fs::read(&path).expect("input");
        let d = Document::open(&path).expect("open");
        let alias = dir.path().join("alias.pdf");
        std::fs::hard_link(&path, &alias).expect("hard link on test volume");
        assert!(d.export_pdf(&alias).is_err());
        assert_eq!(std::fs::read(&path).expect("source after"), source);
        let export = dir.path().join("new.pdf");
        d.export_pdf(&export).expect("export");
        assert!(std::fs::read(&export).expect("pdf").starts_with(b"%PDF-"));
        assert!(d.export_pdf(&export).is_err());
    }
    #[test]
    fn malformed_and_cancelled_inputs_report_errors() {
        let (dir, path) = fixture();
        let broken = dir.path().join("broken.dwg");
        std::fs::write(&broken, b"not a drawing").expect("broken input");
        assert!(Document::open(&broken).is_err());
        assert!(Document::open_with_cancel(&path, Arc::new(AtomicBool::new(true))).is_err());
        assert!(Document::open(&dir.path().join("not-supported.exe")).is_err());
    }
}
