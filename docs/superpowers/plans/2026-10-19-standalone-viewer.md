# LiteCAD Viewer standalone publication plan

**Goal:** Publish 轻图 CAD / LiteCAD Viewer as Dennis-pan's independent application repository and Windows preview release.

**Architecture:** Retain the existing read-only frontend and nine required CADCraft core crates in a self-contained Rust workspace. Exclude the CADCraft editing application, engine, solver, MCP and web frontends. Preserve original licenses and source provenance. The selected origami-bird icon remains the viewer identity.

**Spec:** User-approved product name: 轻图 CAD（LiteCAD Viewer）; account: Dennis-pan-x; display name: Dennis-pan. Features and rendering behavior remain those already accepted by the user.

**Tech stack:** Rust, egui/eframe, wgpu, existing CADCraft readers and renderers.

1. Extract the minimal workspace, rename the frontend and Windows metadata, add standalone documentation, provenance, license notices and CI.
2. Run formatting, clippy and workspace tests using one build job; build a release executable from this workspace. Generate fresh drawing inputs and verify native opening, GPU capture, PDF export, source preservation and icon resources.
3. Rename the existing personal cadcraft repository to litecad-viewer, preserve its history and contribution branch, publish the standalone application on main by a normal fast-forward commit and a Windows ZIP preview with checksums and notices. Verify the remote repository is not a fork and the release can be downloaded.
4. Update the local desktop entry and installed executable to the new product name. Submit a new-application proposal to the ArtCraft launcher project with source, release, screenshot and scope. Inclusion remains a maintainer decision.

**Review focus:** No drawing mutation; no external filesystem path dependencies; no editor dependencies; current icon provenance; honest Windows-only runtime validation and 2D compatibility limits; no claims of official ArtCraft affiliation or inclusion.
