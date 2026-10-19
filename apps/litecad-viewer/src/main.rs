#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented, clippy::unreachable)]
mod camera;
mod diagnostics;
mod document;
mod gpu_view;
mod icons;
mod theme;
mod worker;

use cadcraft_geom::{Bounds2, Vec2};
use cadcraft_render::raster::View;
use cadcraft_render_wgpu as gpu;
use camera::Camera;
use egui::{Color32, Pos2, Rect, Vec2 as ScreenVec};
use icons::Icon;
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};
use worker::{Event, Job, Task};

struct Viewer {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    jobs: Sender<Task>,
    events: Receiver<Event>,
    generation: u64,
    camera: Camera,
    bounds: Bounds2,
    layers: Vec<(String, bool)>,
    layouts: Vec<String>,
    layout: String,
    texture: Option<egui::TextureHandle>,
    image_view: Option<View>,
    desired: Option<View>,
    inflight: bool,
    source: Option<PathBuf>,
    status: String,
    notes: Vec<String>,
    loading: bool,
    loaded: bool,
    dark: bool,
    show_layers: bool,
    layer_filter: String,
    fit: bool,
    dirty: bool,
    measuring: bool,
    point: Option<Vec2>,
    distance: Option<f64>,
    last_size: [u32; 2],
    density: f32,
    capture: Option<PathBuf>,
    capture_sent: bool,
    capture_since: Instant,
    app_mark: Option<egui::TextureHandle>,
    use_gpu: bool,
    gpu_available: bool,
    adapter: String,
    gpu_faults: std::sync::Arc<gpu::GpuFaults>,
    gpu_slot: gpu::MeshSlot,
    gpu_cache: Option<gpu_view::Cache>,
    gpu_key: u64,
    diagnostics: diagnostics::Diagnostics,
}

impl Viewer {
    fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>, capture: Option<PathBuf>, cpu: bool, benchmark: bool, started: Instant) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        #[cfg(windows)]
        for path in ["C:/Windows/Fonts/msyh.ttc", "C:/Windows/Fonts/msjh.ttc", "C:/Windows/Fonts/simhei.ttf"] {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert("viewer-cjk".into(), egui::FontData::from_owned(bytes).into());
                fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "viewer-cjk".into());
                break;
            }
        }
        cc.egui_ctx.set_fonts(fonts);
        theme::apply(&cc.egui_ctx);
        let app_mark = theme::app_icon().map(|icon| {
            cc.egui_ctx.load_texture(
                "cadcraft-app-icon",
                egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba),
                egui::TextureOptions::LINEAR,
            )
        });
        let gpu_faults = std::sync::Arc::new(gpu::GpuFaults::default());
        let mut adapter = "Unavailable".to_owned();
        let device = cc.wgpu_render_state.as_ref().and_then(|rs| {
            let info = rs.adapter.get_info();
            adapter = format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type);
            if cpu || info.device_type == egui_wgpu::wgpu::DeviceType::Cpu {
                return None;
            }
            let installed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gpu::install_with_samples(rs, &gpu_faults, None, 4)));
            (installed.is_ok() && gpu_faults.take().is_none()).then(|| rs.device.clone())
        });
        let gpu_available = device.is_some();
        let (jobs, events, cancel) = worker::spawn(cc.egui_ctx.clone(), device);
        let mut app = Self {
            jobs,
            events,
            cancel,
            generation: 0,
            camera: Camera::default(),
            bounds: Bounds2::EMPTY,
            layers: vec![],
            layouts: vec![],
            layout: String::new(),
            texture: None,
            image_view: None,
            desired: None,
            inflight: false,
            source: None,
            status: "Ready".into(),
            notes: vec![],
            loading: false,
            loaded: false,
            dark: true,
            show_layers: false,
            layer_filter: String::new(),
            fit: true,
            dirty: true,
            measuring: false,
            point: None,
            distance: None,
            last_size: [0, 0],
            density: 1.0,
            capture,
            capture_sent: false,
            capture_since: Instant::now(),
            app_mark,
            use_gpu: gpu_available,
            gpu_available,
            adapter,
            gpu_faults,
            gpu_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),
            gpu_cache: None,
            gpu_key: 0,
            diagnostics: diagnostics::Diagnostics::new(started, benchmark),
        };
        if let Some(path) = path {
            app.open(path);
        }
        app
    }
    fn send(&mut self, job: Job) {
        match &job {
            Job::Render(..) => self.diagnostics.raster_jobs += 1,
            Job::Mesh(..) => self.diagnostics.mesh_jobs += 1,
            _ => {}
        }
        if matches!(job, Job::Open(..) | Job::Space(..)) {
            self.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            self.cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        }
        let token = if matches!(job, Job::Pdf(..)) { std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)) } else { self.cancel.clone() };
        if self.jobs.send(Task { job, cancel: token }).is_err() {
            self.status = "Background worker stopped; restart the viewer.".into();
            self.loading = false;
            self.inflight = false;
        }
    }
    fn open(&mut self, path: PathBuf) {
        self.diagnostics.opening();
        self.gpu_cache = None;
        self.generation = self.generation.wrapping_add(1);
        self.loading = true;
        self.loaded = false;
        self.texture = None;
        self.image_view = None;
        self.inflight = false;
        self.desired = None;
        self.notes.clear();
        self.layers.clear();
        self.layouts.clear();
        self.layout.clear();
        self.point = None;
        self.distance = None;
        self.status = "Opening drawing…".into();
        self.source = Some(path.clone());
        self.send(Job::Open(self.generation, path));
    }
    fn pick_open(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("CAD drawings", &["dwg", "dxf", "dwt"]).pick_file() {
            self.open(p);
        }
    }
    fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Loaded(id, m, ms) if id == self.generation => {
                    self.diagnostics.read_ms = m.timings.read_ms;
                    self.diagnostics.parse_ms = m.timings.parse_ms;
                    self.diagnostics.list_ms = m.timings.list_ms;
                    self.bounds = m.bounds;
                    self.layers = m.layers;
                    self.layouts = m.layouts;
                    self.notes = m.notes;
                    self.loading = false;
                    self.loaded = true;
                    self.fit = true;
                    self.dirty = true;
                    self.status = format!("Read only · {} objects · {:.0} ms", m.entities, ms);
                    if let Some(p) = &self.source {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                            "{} — LiteCADViewer",
                            p.file_name().unwrap_or_default().to_string_lossy()
                        )));
                    }
                }
                Event::Changed(id, b, refit) if id == self.generation => {
                    self.gpu_cache = None;
                    self.bounds = b;
                    self.dirty = true;
                    if refit {
                        self.fit = true;
                        self.loading = false;
                        self.texture = None;
                        self.image_view = None;
                        self.point = None;
                        self.distance = None;
                    }
                }
                Event::Image(id, v, pixels, ms) if id == self.generation => {
                    let colors = pixels.as_chunks::<4>().0.iter().map(|p| Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3])).collect();
                    let image = egui::ColorImage::new([v.width as usize, v.height as usize], colors);
                    self.texture = Some(ctx.load_texture("drawing", image, egui::TextureOptions::LINEAR));
                    self.image_view = Some(v);
                    self.inflight = false;
                    self.status = format!("Read only · {:.0} ms render", ms);
                }
                Event::Mesh(id, frame, ms) if id == self.generation => {
                    self.inflight = false;
                    if self.use_gpu {
                        let frame = *frame;
                        if let Ok(mut slot) = self.gpu_slot.lock() {
                            *slot = Some(frame.data);
                        }
                        self.gpu_cache = Some(frame.cache);
                        self.gpu_key = self.gpu_key.wrapping_add(1);
                        self.diagnostics.mesh_prepare_ms += ms;
                        self.status = format!("Read only · GPU · {:.0} ms", ms);
                    }
                }
                Event::Aborted(id) if id == self.generation => {
                    self.inflight = false;
                }
                Event::Error(id, e) if id == self.generation => {
                    self.status = e;
                    self.loading = false;
                    self.inflight = false;
                }
                Event::Saved(id, p) if id == self.generation => {
                    self.status = format!("PDF: {}", p.display());
                }
                _ => {}
            }
        }
    }
    fn save_capture(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.raw.events.clone());
        for e in events {
            if let egui::Event::Screenshot { image, .. } = e
                && let Some(path) = self.capture.take()
            {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Err(e) = save_png(&path, &bytes, image.width() as u32, image.height() as u32) {
                    self.status = e.to_string();
                }
                match serde_json::to_vec_pretty(&self.diagnostics.report(self.use_gpu, &self.adapter)) {
                    Ok(bytes) => {
                        if let Err(e) = document::write_new(&path.with_extension("json"), &bytes) {
                            self.status = e;
                        }
                    }
                    Err(e) => self.status = e.to_string(),
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        let ready = if self.use_gpu { self.gpu_cache.is_some() && self.diagnostics.gpu_frames > 0 } else { self.texture.is_some() };
        if self.capture.is_some() && ready && !self.inflight && !self.dirty && !self.capture_sent && self.diagnostics.finished() {
            self.capture_sent = true;
            self.capture_since = Instant::now();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        if self.capture.is_some() && self.capture_since.elapsed() > Duration::from_secs(if self.capture_sent { 15 } else { 90 }) {
            if let Some(p) = self.capture.take() {
                let _ = document::write_new(&p.with_extension("error.txt"), b"Capture did not complete; opening failed or no frame was presented.");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for Viewer {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);
        if let Some(fault) = self.gpu_faults.take() {
            self.use_gpu = false;
            self.gpu_available = false;
            self.gpu_cache = None;
            self.inflight = false;
            self.dirty = true;
            self.notes.push(format!("GPU drawing disabled; using CPU raster: {}", fault.message));
            if self.loaded {
                self.loading = true;
                self.send(Job::Space(self.generation, self.layout.clone()));
            }
        }
        let ready = self.loaded
            && !self.loading
            && !self.inflight
            && !self.dirty
            && if self.use_gpu { self.gpu_cache.is_some() && self.diagnostics.gpu_frames > 0 } else { self.texture.is_some() };
        if ready && self.diagnostics.ready(&mut self.camera) {
            self.dirty = true;
        }
        self.save_capture(ctx);
        for f in ctx.input(|i| i.raw.dropped_files.clone()) {
            let p = f.path();
            if !p.as_os_str().is_empty() {
                self.open(p.to_path_buf());
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.pick_open();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
            self.fit = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.measuring = false;
            self.point = None;
        }
        if self.capture.is_some() && self.diagnostics.benchmark && !self.capture_sent {
            ctx.request_repaint();
        } else if self.loading || self.inflight || self.capture.is_some() {
            ctx.request_repaint_after(Duration::from_millis(40));
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("toolbar").frame(egui::Frame::new().fill(theme::CHROME).inner_margin(8.0)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if let Some(mark) = &self.app_mark {
                    ui.image((mark.id(), egui::vec2(26.0, 26.0)));
                }
                ui.strong("LiteCADViewer");
                ui.separator();
                if icons::button(ui, Icon::Open, "Open", false, true).on_hover_text("Ctrl+O / DWG, DXF, DWT").clicked() {
                    self.pick_open();
                }
                if icons::button(ui, Icon::Fit, "Fit", false, self.loaded).on_hover_text("Home / Double click").clicked() {
                    self.fit = true;
                }
                if icons::button(ui, Icon::Layers, "Layers", self.show_layers, true).clicked() {
                    self.show_layers = !self.show_layers;
                }
                if icons::button(ui, Icon::Measure, "Measure", self.measuring, self.loaded).clicked() {
                    self.measuring = !self.measuring;
                    self.point = None;
                }
                if icons::button(ui, Icon::Sun, "Dark / light", !self.dark, true).clicked() {
                    self.dark = !self.dark;
                    self.dirty = true;
                }
                if icons::button(ui, Icon::Export, "Export PDF", false, self.loaded && !self.loading).clicked()
                    && let Some(p) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name("drawing.pdf").save_file()
                {
                    self.send(Job::Pdf(self.generation, p));
                }
                if ui
                    .add_enabled(self.gpu_available && !self.inflight && !self.loading, egui::Checkbox::new(&mut self.use_gpu, "GPU"))
                    .on_hover_text(&self.adapter)
                    .changed()
                {
                    self.gpu_cache = None;
                    self.desired = None;
                    self.dirty = true;
                }
            });
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                let title = self
                    .source
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Drop a DWG / DXF drawing to begin".into());
                let path = self.source.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                ui.add(egui::Label::new(egui::RichText::new(title).small().color(theme::TEXT_DIM)).truncate()).on_hover_text(path);
                ui.separator();
                ui.small("Read-only preview");
            });
        });
        egui::Panel::bottom("status").frame(egui::Frame::new().fill(theme::CHROME).inner_margin(6.0)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self.loading {
                    ui.spinner();
                }
                ui.small(&self.status);
                ui.separator();
                ui.small(if self.use_gpu { "GPU" } else { "CPU" }).on_hover_text(&self.adapter);
                ui.separator();
                ui.small("Wheel zoom · Drag to pan · Double-click to fit");
                if let Some(d) = self.distance {
                    ui.strong(format!("Distance {:.4} drawing units", d));
                }
            });
        });
        if self.loaded {
            let previous = self.layout.clone();
            egui::Panel::bottom("spaces").frame(egui::Frame::new().fill(theme::CHROME_DARK).inner_margin(5.0)).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Space");
                    ui.selectable_value(&mut self.layout, String::new(), "Model");
                    for layout in &self.layouts {
                        ui.selectable_value(&mut self.layout, layout.clone(), layout);
                    }
                });
            });
            if self.layout != previous {
                self.loading = true;
                self.send(Job::Space(self.generation, self.layout.clone()));
            }
        }
        egui::Panel::top("notice").frame(egui::Frame::new().fill(theme::CHROME_DARK).inner_margin(6.0)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(theme::WARN, "2D preview · Source unchanged · Verify SHX, custom objects and 3D in another viewer");
            });
            if !self.notes.is_empty() {
                egui::CollapsingHeader::new(format!("Read notes ({})", self.notes.len())).default_open(true).show(ui, |ui| {
                    for n in &self.notes {
                        ui.label(n);
                    }
                });
            }
        });
        if self.show_layers {
            egui::Panel::right("layers").default_size(230.0).resizable(true).show(ui, |ui| {
                ui.heading("Layers");
                ui.text_edit_singleline(&mut self.layer_filter);
                let mut changed = vec![];
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (name, on) in &mut self.layers {
                        if name.to_lowercase().contains(&self.layer_filter.to_lowercase()) && ui.checkbox(on, name.as_str()).changed() {
                            changed.push((name.clone(), *on));
                        }
                    }
                });
                if !changed.is_empty() {
                    self.send(Job::Layers(self.generation, changed));
                }
            });
        }
        let bg = if self.dark && self.layout.is_empty() { theme::CANVAS } else { theme::PAPER };
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(bg)).show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
            let (w, h, ppp) = if self.use_gpu {
                let ppp = ui.ctx().pixels_per_point();
                ((rect.width() * ppp).ceil().clamp(1.0, 65536.0) as u32, (rect.height() * ppp).ceil().clamp(1.0, 65536.0) as u32, ppp)
            } else {
                camera::raster_size(rect.width(), rect.height(), ui.ctx().pixels_per_point())
            };
            let size = [w, h];
            if !self.fit && self.density != ppp {
                self.camera.scale *= f64::from(ppp / self.density);
                self.dirty = true;
            }
            self.density = ppp;
            if self.loaded && !self.loading && (self.fit || self.last_size != size) {
                if self.fit {
                    let v = View::fit(&self.bounds, size[0], size[1], 0.08);
                    self.camera = Camera { x: v.center.x, y: v.center.y, scale: v.scale };
                    self.fit = false;
                }
                self.last_size = size;
                self.dirty = true;
            }
            if response.double_clicked() {
                self.fit = true;
            }
            if self.loaded && !self.loading {
                if response.dragged() && !self.measuring {
                    let delta = ui.input(|i| i.pointer.delta());
                    self.camera.pan(f64::from(delta.x * ppp), f64::from(delta.y * ppp));
                    self.dirty = true;
                }
                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0
                        && let Some(p) = response.hover_pos()
                    {
                        let offset = (p - rect.center()) * ppp;
                        self.camera.zoom((f64::from(scroll) * 0.003).exp().clamp(0.5, 2.0), f64::from(offset.x), f64::from(offset.y));
                        self.dirty = true;
                    }
                    if self.measuring
                        && response.clicked()
                        && let Some(p) = response.interact_pointer_pos()
                    {
                        let o = (p - rect.center()) * ppp;
                        let (x, y) = self.camera.world(f64::from(o.x), f64::from(o.y));
                        let point = Vec2::new(x, y);
                        if let Some(a) = self.point.take() {
                            self.distance = Some(((point.x - a.x).powi(2) + (point.y - a.y).powi(2)).sqrt());
                        } else {
                            self.point = Some(point);
                            self.distance = None;
                        }
                    }
                }
                let view = View { center: Vec2::new(self.camera.x, self.camera.y), scale: self.camera.scale, width: size[0], height: size[1] };
                if self.desired != Some(view) {
                    self.desired = Some(view);
                    self.dirty = true;
                }
                // Cached GPU geometry only needs a new view uniform while navigating.
                if self.dirty && !self.inflight {
                    self.dirty = false;
                    if !self.use_gpu || self.gpu_cache.as_ref().is_none_or(|cache| !cache.serves(view, self.dark)) {
                        self.inflight = true;
                        self.send(if self.use_gpu {
                            Job::Mesh(self.generation, view, self.dark)
                        } else {
                            Job::Render(self.generation, view, self.dark)
                        });
                    }
                }
                if self.use_gpu
                    && let Some(cache) = &self.gpu_cache
                {
                    let center = rect.center();
                    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                        rect,
                        gpu::CanvasCallback {
                            key: self.gpu_key,
                            slot: self.gpu_slot.clone(),
                            offset: [(cache.origin.x - view.center.x) as f32, (cache.origin.y - view.center.y) as f32],
                            scale: (view.scale / f64::from(ppp)) as f32,
                            center: [center.x, center.y],
                            layer: gpu::LAYER_DRAWING,
                            xform: gpu::IDENTITY_XFORM,
                            dash: 0.0,
                            stencil: gpu::Stencil::Off,
                        },
                    ));
                    self.diagnostics.gpu_frames += 1;
                } else if !self.use_gpu
                    && let (Some(texture), Some(old)) = (&self.texture, self.image_view)
                {
                    let ratio = self.camera.scale / old.scale;
                    let center = rect.center()
                        + ScreenVec::new(
                            ((old.center.x - self.camera.x) * self.camera.scale / f64::from(ppp)) as f32,
                            ((self.camera.y - old.center.y) * self.camera.scale / f64::from(ppp)) as f32,
                        );
                    let extent = ScreenVec::new(old.width as f32 / ppp, old.height as f32 / ppp) * ratio as f32;
                    ui.painter().image(
                        texture.id(),
                        Rect::from_center_size(center, extent),
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                if let Some(a) = self.point {
                    let p = rect.center()
                        + ScreenVec::new(
                            ((a.x - self.camera.x) * self.camera.scale / f64::from(ppp)) as f32,
                            ((self.camera.y - a.y) * self.camera.scale / f64::from(ppp)) as f32,
                        );
                    ui.painter().circle_stroke(p, 5.0, (2.0, Color32::from_rgb(255, 185, 65)));
                }
            } else if !self.loading {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Drop a drawing here, or choose Open",
                    egui::FontId::proportional(20.0),
                    Color32::from_rgb(150, 160, 174),
                );
            }
        });
    }
}

fn save_png(path: &std::path::Path, bytes: &[u8], width: u32, height: u32) -> Result<(), String> {
    use image::ImageEncoder;
    if path.extension().and_then(|s| s.to_str()).is_none_or(|s| !s.eq_ignore_ascii_case("png")) {
        return Err("Screenshot output must have a .png extension.".into());
    }
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded)
        .write_image(bytes, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    document::write_new(path, &encoded)
}

fn probe(path: PathBuf, output: PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let start = Instant::now();
    let mut d = document::Document::open(&path)?;
    let open_ms = start.elapsed().as_secs_f64() * 1000.0;
    let view = View::fit(&d.list.bounds, 1600, 1000, 0.08);
    let render = Instant::now();
    let rgba = d.render(view, true)?;
    let render_ms = render.elapsed().as_secs_f64() * 1000.0;
    let colors: Vec<Color32> = rgba.as_chunks::<4>().0.iter().map(|p| Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3])).collect();
    let pixels: Vec<u8> = colors.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    save_png(&output.join("drawing.png"), &pixels, view.width, view.height).map_err(|e| e.to_string())?;
    d.export_pdf(&output.join("drawing.pdf"))?;
    let before = d.list.prims.len();
    let layer = d.drawing.layers.iter().find(|l| l.name == "DETAIL").map(|l| l.name.clone());
    let hidden = if let Some(name) = layer {
        d.set_layer(&name, false)?;
        Some(d.list.prims.len())
    } else {
        None
    };
    let report = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"source":path,"open_ms":open_ms,"render_ms":render_ms,"entities":d.drawing.entity_count(),"primitives":before,"primitives_after_hiding_DETAIL":hidden,"notes":d.notes,"editing_engine_linked":false,"space":d.space});
    document::write_new(&output.join("result.json"), &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> eframe::Result {
    let started = Instant::now();
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut capture = None;
    let mut cpu = false;
    let mut benchmark = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--probe" => {
                if let (Some(p), Some(o)) = (args.next(), args.next()) {
                    if let Err(e) = probe(p.into(), o.into()) {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                } else {
                    std::process::exit(2);
                }
                return Ok(());
            }
            "--capture" => capture = args.next().map(PathBuf::from),
            "--cpu" => cpu = true,
            "--benchmark" => benchmark = true,
            "--lang" => {
                // Accept legacy shortcuts; this edition always uses English.
                let _ = args.next();
            }
            "--version" => {
                println!("LiteCADViewer {} (read-only)", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => path = Some(PathBuf::from(a)),
        }
    }
    let mut options = eframe::NativeOptions {
        multisampling: 4,
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 820.0]).with_min_inner_size([760.0, 500.0]).with_drag_and_drop(true),
        ..Default::default()
    };
    if let Some(icon) = theme::app_icon() {
        options.viewport = options.viewport.with_icon(icon);
    }
    #[cfg(windows)]
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup {
        create.instance_descriptor.backends = eframe::wgpu::Backends::from_env().unwrap_or(eframe::wgpu::Backends::DX12);
        create.instance_descriptor.backend_options.dx12.shader_compiler = eframe::wgpu::Dx12Compiler::Fxc;
    }
    eframe::run_native("LiteCADViewer", options, Box::new(move |cc| Ok(Box::new(Viewer::new(cc, path, capture, cpu, benchmark, started)))))
}
