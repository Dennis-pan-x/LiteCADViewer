//! Native first-view and settled-view diagnostics; these are wall times, not GPU timestamps.
use crate::camera::Camera;
use std::time::Instant;

pub struct Diagnostics {
    started: Instant,
    opened: Instant,
    pub initialization_ms: f64,
    pub read_ms: f64,
    pub parse_ms: f64,
    pub list_ms: f64,
    pub mesh_prepare_ms: f64,
    pub mesh_jobs: u64,
    pub raster_jobs: u64,
    pub gpu_frames: u64,
    pub first_view_ms: Option<f64>,
    pub drawing_first_view_ms: Option<f64>,
    pub benchmark: bool,
    step: Option<Instant>,
    samples: Vec<f64>,
}
impl Diagnostics {
    pub fn new(started: Instant, benchmark: bool) -> Self {
        Self {
            started,
            opened: Instant::now(),
            initialization_ms: started.elapsed().as_secs_f64() * 1000.0,
            read_ms: 0.0,
            parse_ms: 0.0,
            list_ms: 0.0,
            mesh_prepare_ms: 0.0,
            mesh_jobs: 0,
            raster_jobs: 0,
            gpu_frames: 0,
            first_view_ms: None,
            drawing_first_view_ms: None,
            benchmark,
            step: None,
            samples: vec![],
        }
    }
    pub fn opening(&mut self) {
        self.opened = Instant::now();
    }
    /// Call only after the current view was submitted and all its worker work settled.
    pub fn ready(&mut self, camera: &mut Camera) -> bool {
        if self.first_view_ms.is_none() {
            self.first_view_ms = Some(self.started.elapsed().as_secs_f64() * 1000.0);
            self.drawing_first_view_ms = Some(self.opened.elapsed().as_secs_f64() * 1000.0);
        }
        if !self.benchmark || self.samples.len() >= 30 {
            return false;
        }
        if let Some(started) = self.step.take() {
            self.samples.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        if self.samples.len() >= 30 {
            return false;
        }
        camera.pan(if self.samples.len().is_multiple_of(2) { 8.0 } else { -8.0 }, 0.0);
        camera.zoom(1.002, 0.0, 0.0);
        self.step = Some(Instant::now());
        true
    }
    pub fn finished(&self) -> bool {
        !self.benchmark || self.samples.len() >= 30
    }
    pub fn report(&self, gpu: bool, adapter: &str) -> serde_json::Value {
        let mut sorted = self.samples.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted.get(sorted.len() / 2).copied();
        let p95 = sorted.get(sorted.len().saturating_mul(95) / 100).copied();
        serde_json::json!({"geometry_backend":if gpu {"gpu"}else{"cpu"},"adapter":adapter,
            "initialization_ms":self.initialization_ms,"file_read_ms":self.read_ms,"parse_ms":self.parse_ms,
            "display_list_ms":self.list_ms,"mesh_prepare_ms":self.mesh_prepare_ms,
            "total_first_view_ms":self.first_view_ms,"drawing_first_view_ms":self.drawing_first_view_ms,
            "gpu_draw_frames":self.gpu_frames,"gpu_mesh_jobs":self.mesh_jobs,"cpu_raster_jobs":self.raster_jobs,
            "settled_view_updates":self.samples.len(),"settled_view_median_ms":median,"settled_view_p95_ms":p95,
            "timing_note":"Native wall-clock settled view latency, including scheduling/presentation; not GPU execution timestamps."})
    }
}
