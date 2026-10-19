use crate::document::Document;
use cadcraft_geom::Bounds2;
use cadcraft_render::raster::View;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::Instant,
};

pub enum Job {
    Open(u64, PathBuf),
    Layers(u64, Vec<(String, bool)>),
    Space(u64, String),
    Render(u64, View, bool),
    Mesh(u64, View, bool),
    Pdf(u64, PathBuf),
}
pub struct Task {
    pub job: Job,
    pub cancel: Arc<AtomicBool>,
}
pub struct Meta {
    pub bounds: Bounds2,
    pub layers: Vec<(String, bool)>,
    pub layouts: Vec<String>,
    pub notes: Vec<String>,
    pub entities: usize,
    pub timings: crate::document::OpenTimings,
}
pub enum Event {
    Loaded(u64, Meta, f64),
    Changed(u64, Bounds2, bool),
    Image(u64, View, Vec<u8>, f64),
    Mesh(u64, Box<crate::gpu_view::Frame>, f64),
    Saved(u64, PathBuf),
    Error(u64, String),
    Aborted(u64),
}

pub fn spawn(ctx: egui::Context, device: Option<egui_wgpu::wgpu::Device>) -> (Sender<Task>, Receiver<Event>, Arc<AtomicBool>) {
    let (tx, rx) = mpsc::channel::<Task>();
    let (events, result) = mpsc::channel();
    let initial = Arc::new(AtomicBool::new(false));
    std::thread::spawn(move || {
        let mut doc: Option<Document> = None;
        let mut active = 0;
        let mut pending = std::collections::VecDeque::new();
        while let Some(mut task) = pending.pop_front().or_else(|| rx.recv().ok()) {
            pending.extend(rx.try_iter());
            // Tokens are immutable per request group: the worker never clears a
            // cancellation signal. Explicit exports receive independent tokens.
            let id = match &task.job {
                Job::Open(id, _) | Job::Layers(id, ..) | Job::Space(id, _) | Job::Render(id, ..) | Job::Mesh(id, ..) | Job::Pdf(id, _) => *id,
            };
            if task.cancel.load(Ordering::Relaxed) && !matches!(task.job, Job::Layers(..)) {
                let _ = events.send(Event::Aborted(id));
                ctx.request_repaint();
                continue;
            }
            if !matches!(task.job, Job::Open(..)) && id != active {
                continue;
            }
            if let Job::Layers(id, changes) = &mut task.job {
                while matches!(pending.front(),Some(Task{job:Job::Layers(next,_),cancel}) if next==id && Arc::ptr_eq(cancel,&task.cancel)) {
                    if let Some(Task { job: Job::Layers(_, mut more), .. }) = pending.pop_front() {
                        changes.append(&mut more);
                    }
                }
            }
            let start = Instant::now();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Event, String> {
                match task.job {
                    Job::Open(id, path) => {
                        let d = Document::open_with_cancel(&path, task.cancel.clone())?;
                        let meta = Meta {
                            bounds: d.list.bounds,
                            layers: d.drawing.layers.iter().map(|l| (l.name.clone(), l.visible())).collect(),
                            layouts: d.drawing.layouts.iter().map(|l| l.name.clone()).collect(),
                            notes: d.notes.clone(),
                            entities: d.drawing.entity_count(),
                            timings: d.timings,
                        };
                        doc = Some(d);
                        active = id;
                        Ok(Event::Loaded(id, meta, start.elapsed().as_secs_f64() * 1000.0))
                    }
                    other => {
                        let d = doc.as_mut().ok_or("Open a drawing first.")?;
                        d.use_cancel(task.cancel.clone());
                        match other {
                            Job::Layers(id, changes) => {
                                d.apply_layers(&changes);
                                d.rebuild()?;
                                Ok(Event::Changed(id, d.list.bounds, false))
                            }
                            Job::Space(id, name) => {
                                d.set_space(&name)?;
                                Ok(Event::Changed(id, d.list.bounds, true))
                            }
                            Job::Render(id, view, dark) => {
                                let pixels = d.render(view, dark)?;
                                Ok(Event::Image(id, view, pixels, start.elapsed().as_secs_f64() * 1000.0))
                            }
                            Job::Mesh(id, view, dark) => {
                                let frame = crate::gpu_view::build(d, view, dark, device.as_ref())?;
                                Ok(Event::Mesh(id, Box::new(frame), start.elapsed().as_secs_f64() * 1000.0))
                            }
                            Job::Pdf(id, path) => {
                                d.export_pdf(&path)?;
                                Ok(Event::Saved(id, path))
                            }
                            Job::Open(..) => Err("Unexpected open request.".into()),
                        }
                    }
                }
            }));
            let event = match outcome {
                Ok(Ok(e)) => e,
                Ok(Err(e)) if e == "Superseded request." => Event::Aborted(id),
                Ok(Err(e)) => Event::Error(id, e),
                Err(_) => {
                    doc = None;
                    Event::Error(id, "Drawing processing failed. Open another drawing to continue.".into())
                }
            };
            if events.send(event).is_err() {
                break;
            }
            ctx.request_repaint();
        }
    });
    (tx, result, initial)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn input(dir: &std::path::Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let mut drawing = cadcraft_doc::Drawing::new_metric();
        drawing
            .add(
                &cadcraft_doc::Space::Model,
                cadcraft_doc::Common { layer: "DETAIL".into(), ..Default::default() },
                cadcraft_doc::EntityKind::Line(cadcraft_doc::Line { a: cadcraft_geom::Vec3::ZERO, b: cadcraft_geom::Vec3::new(10.0, 20.0, 0.0) }),
            )
            .expect("fresh line");
        std::fs::write(&path, cadcraft_io::write(&drawing, name).expect("fresh drawing")).expect("input");
        path
    }
    fn token() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }
    #[test]
    fn cancelled_requests_never_reset_their_token_or_block_the_next_open() {
        let dir = tempfile::tempdir().expect("temp");
        let p = input(dir.path(), "new.dxf");
        let (tx, rx, _) = spawn(egui::Context::default(), None);
        let cancelled = Arc::new(AtomicBool::new(true));
        tx.send(Task { job: Job::Open(1, dir.path().join("does-not-exist.dwg")), cancel: cancelled.clone() }).expect("queue");
        tx.send(Task { job: Job::Open(2, p), cancel: token() }).expect("queue");
        assert!(matches!(rx.recv_timeout(Duration::from_secs(15)).expect("cancelled"), Event::Aborted(1)));
        assert!(matches!(rx.recv_timeout(Duration::from_secs(15)).expect("opened"), Event::Loaded(2, ..)));
        assert!(cancelled.load(Ordering::Relaxed));
    }
    #[test]
    fn export_before_open_is_not_silently_dropped() {
        let dir = tempfile::tempdir().expect("temp");
        let a = input(dir.path(), "a.dxf");
        let b = input(dir.path(), "b.dxf");
        let pdf = dir.path().join("export.pdf");
        let (tx, rx, _) = spawn(egui::Context::default(), None);
        let a_token = token();
        tx.send(Task { job: Job::Open(1, a), cancel: a_token.clone() }).expect("open a");
        assert!(matches!(rx.recv_timeout(Duration::from_secs(15)).expect("opened a"), Event::Loaded(1, ..)));
        tx.send(Task { job: Job::Pdf(1, pdf.clone()), cancel: token() }).expect("export");
        a_token.store(true, Ordering::Relaxed);
        tx.send(Task { job: Job::Open(2, b), cancel: token() }).expect("open b");
        assert!(matches!(rx.recv_timeout(Duration::from_secs(15)).expect("saved"), Event::Saved(1, ..)));
        assert!(matches!(rx.recv_timeout(Duration::from_secs(15)).expect("opened b"), Event::Loaded(2, ..)));
        assert!(std::fs::read(pdf).expect("pdf").starts_with(b"%PDF-"));
    }

    #[test]
    fn cancelling_a_layer_rebuild_still_applies_visibility_before_navigation() {
        let dir = tempfile::tempdir().expect("temp");
        let path = input(dir.path(), "layers.dxf");
        let (tx, rx, _) = spawn(egui::Context::default(), None);
        tx.send(Task { job: Job::Open(1, path), cancel: token() }).expect("open");
        assert!(matches!(rx.recv_timeout(Duration::from_secs(15)).expect("loaded"), Event::Loaded(1, ..)));
        tx.send(Task { job: Job::Layers(1, vec![("DETAIL".into(), false)]), cancel: Arc::new(AtomicBool::new(true)) }).expect("layer");
        tx.send(Task { job: Job::Space(1, String::new()), cancel: token() }).expect("navigate");
        assert!(matches!(rx.recv_timeout(Duration::from_secs(15)).expect("cancelled rebuild"), Event::Aborted(1)));
        match rx.recv_timeout(Duration::from_secs(15)).expect("changed") {
            Event::Changed(1, b, true) => assert!(b.is_empty()),
            _ => panic!("expected refitted empty model after hiding the only layer"),
        }
    }
}
