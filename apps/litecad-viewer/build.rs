//! Embed the original viewer icon, distinct from the CADCraft editor icon.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/viewer-icon/viewer.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../../assets/viewer-icon/viewer.ico");
    resource
        .set("ProductName", "LiteCADViewer")
        .set("FileDescription", "LiteCADViewer lightweight read-only CAD viewer")
        .set("CompanyName", "Dennis-pan and CADCraft contributors")
        .set("LegalCopyright", "Copyright (c) 2026 Dennis-pan and CADCraft contributors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "LiteCADViewer.exe")
        .set("InternalName", "litecad-viewer");
    if let Err(error) = resource.compile() {
        eprintln!("embedding viewer icon/version resources failed: {error}");
        std::process::exit(1);
    }
}
