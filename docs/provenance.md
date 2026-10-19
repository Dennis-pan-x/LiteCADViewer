# Source provenance

LiteCADViewer is an independent community application maintained by Dennis-pan (@Dennis-pan-x). It is not the CADCraft design editor and is not an official ArtCraft product.

The initial source snapshot is derived from [CADCraft](https://github.com/storytold/cadcraft), v0.7.0 base `fc4e98315a8d771b5ab832f0ade0b5e7b106df7f`, with the viewer/GPU work in Dennis-pan's [contribution commit](https://github.com/Dennis-pan-x/LiteCADViewer/commit/f413eec53018504421de7089d208f1b62447e0ff). The related upstream component PR is [#1606](https://github.com/storytold/cadcraft/pull/1606); it is not the independent application's repository.

The `geom`, `dxf`, `dwg`, `color`, `doc`, `fonts`, `render` and `io` crates retain CADCraft implementations. `render-wgpu` is the engine-free backend extracted during viewer development. The frontend lives in `apps/litecad-viewer`; it has its own product name, application version, icon and releases. The full CADCraft engine, editing UI, constraints, MCP, command-line editor and web app are not included.

Upstream MIT and Apache-2.0 licenses and NOTICE are retained. New viewer contributions and icon assets are also MIT OR Apache-2.0. The unmodified `acadrust` dependency remains MPL-2.0; its source is available from its package repository and source links in the release's third-party notices. No claim is made that Dennis-pan authored the CADCraft core.
