# LiteCADViewer

`litecad-viewer` is a separate native frontend for opening and reviewing DWG,
DXF and DWT drawings. It reuses the existing `io`, `doc`, `fonts` and `render`
crates. It does **not** depend on the editing engine, constraint solver or the
editor UI. The full CADCraft desktop app is unchanged.

```sh
cargo run --release -p litecad-viewer -- drawing.dwg
```

The interface and application documentation are English-only. Legacy `--lang`
arguments are accepted for shortcut compatibility but do not change the interface.
Drawing text, filenames and layer names retain their original Unicode content.
On Windows the viewer can use installed YaHei, JhengHei or SimHei fonts for these
names; it never ships copies of system fonts.

The chrome follows CADCraft's slate palette, typography, compact controls and
24-pixel line-icon geometry. Toolbar controls use the same `craft-ui` icon-button
component as the editor; no editing engine is loaded. The window, executable and
desktop shortcut use original MIT/Apache-licensed viewer artwork from
`assets/viewer-icon`: an amber origami bird, drafting tools and drawing sheet on a
cyan tile, sharing the family palette while remaining visually distinct from the
editor's icon. The high-resolution RGBA PNG is the source master; PNG/ICO exports
include sizes down to 16 pixels. Restricted
ArtCraft marks from `docs/brand` are not bundled.

## Reviewing drawings

- Open (`Ctrl+O`) or drop a drawing into the window. One drawing per window.
- Zoom with the wheel, drag with the left/middle mouse button to pan, and
  double-click or press `Home` to fit the drawing.
- Show the Layers panel to filter names and switch visibility in memory.
- Choose Model or a layout tab. Switching spaces fits the new space.
- Measure two freely picked points. Results use drawing units, without object
  snaps; this is a visual review tool, not precision drafting.
- Export the active space to a new PDF. Existing output files are deliberately
  refused rather than truncated, including hard links to a source drawing.

The viewer provides no drawing save, entity editing, command execution or
autosave. Layer visibility, active space and camera changes are transient.
Read notes and unsupported/proxy-object warnings remain visible. Source drawings
are opened for reading only.

## Performance and limits

The viewer defaults to GPU drawing on a hardware adapter, using the editor's
backend extracted into `cadcraft-render-wgpu`. This shared L5 adapter has no
editing-engine dependency; the upstream editor can re-export the same API. The viewer
uses four-sample antialiasing. The GPU checkbox/status identifies the active
drawing backend; its tooltip names the actual adapter. Uncheck it or launch with
`--cpu` to use CPU rasterization. Reported drawing errors disable GPU drawing;
complete device loss may still require restarting the app.

Parsing, display-list builds, GPU mesh preparation/upload and PDF output run on
the worker. Once geometry is uploaded, ordinary pan/zoom only updates view
uniforms. A new mesh is requested when layers, space, background, curve precision
or the vertex-origin precision budget changes. The existing mesh stays visible
while a finer one is prepared. Primitive-order groups preserve transparent fills
and wipeouts; XLINE/RAY extend in the shader inside those same groups.

Initial curve tolerance follows approximate drawing extents for a 4096-pixel
fit, rather than a fixed tiny drawing-unit error. Deep zoom refines curves when
their chord error exceeds half a pixel. File parsing remains CPU work: GPU
drawing does not promise faster parsing or better format compatibility.

In CPU mode a cached display list is reused; only one raster request is in flight,
and the latest desired view is retained while the previous image follows the
camera. CPU raster dimensions are bounded uniformly to 4096 pixels per side.
Layer changes are batched; superseded operations use per-request cancellation
tokens. DWG parsing itself is not yet interruptible. Explicit PDF exports keep
their queue position. Do not infer a speed improvement for every drawing.

This is a **2D preview**, with the same reader/font/entity limitations as the
current CADCraft core. SHX/big fonts, custom objects and 3D appearance need
independent verification. It does not claim AutoCAD or DWG FastView parity.
File inputs are limited to 256 MiB. Rust panics from processing are caught, but
process aborts and allocation exhaustion are not isolated.

## Validation

```sh
cargo test -p litecad-viewer
cargo clippy -p litecad-viewer --all-targets -- -D warnings
cargo build --release -p litecad-viewer
```

Tests generate fresh drawings in temporary directories. They cover cursor-fixed
zoom at large world coordinates, high-DPI coverage, pan, layer visibility and
layout bounds, malformed/cancelled requests, preserving input bytes, output
hard links, preserving an explicit PDF export while another drawing opens,
screen-bounded initial curves, deep-zoom mesh reuse, infinite lines and ordered fills.

For local diagnostics (use a **new** output directory each time):

```sh
litecad-viewer --probe drawing.dxf new-output-directory
litecad-viewer --capture new-window.png --lang en drawing.dxf
litecad-viewer --capture new-gpu-benchmark.png --benchmark drawing.dxf
litecad-viewer --capture new-cpu-benchmark.png --benchmark --cpu drawing.dxf
```

`--probe` writes a drawing PNG, PDF and JSON timings through the same reader and
rasterizer as the GUI. It is not a GUI startup benchmark or an independent
rendering oracle. `--capture` uses egui's screenshot event and closes its test
window afterward. A JSON sidecar reports the actual adapter/backend, stage timings,
first-view latency and GPU mesh/CPU raster job counts. `--benchmark` additionally
performs 30 settled pan/zoom updates. Its wall-clock latency includes scheduling
and presentation; it is not a GPU execution timestamp. If no native frame is
presented, an error sidecar is written. All diagnostic outputs refuse replacement.

The native app has been developed and tested on Windows; other supported desktop
platforms require their own runtime acceptance. No additional proprietary assets,
fonts or binary fixtures are included.
