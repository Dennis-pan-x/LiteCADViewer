# LiteCADViewer contributor guidance

This is the independent LiteCADViewer application, maintained by Dennis-pan (@Dennis-pan-x). It derives nine permissively licensed core crates from CADCraft; provenance is recorded in docs/provenance.md. Keep source copyrights and notices.

- Keep the frontend read-only: never add source drawing writes, autosave or overwriting export outputs.
- Avoid editing-engine, constraints and full-editor dependencies. Keep GPU caching and ordered drawing behavior intact.
- Never add proprietary CAD fonts, icons, binaries, patterns or code. Original assets require ATTRIBUTION.md entries.
- No unsafe Rust or production unwrap/expect/panic. Validate untrusted drawing sizes, numeric values and recursion.
- Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings and cargo test --workspace before commits. Use CARGO_BUILD_JOBS=1 when memory is constrained.
- Viewer changes require a release build and a native fresh synthetic drawing capture. Distinguish synthetic evidence from independent CAD compatibility.
- Public wording must say community application derived from CADCraft; do not imply official ArtCraft affiliation or launcher inclusion.
