//! Windows: embed the app icon and version info into librarian.exe.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    // Only when building *on* Windows for Windows (needs the MSVC resource
    // compiler); cross-checks from macOS skip it.
    if cfg!(windows) && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Librarian");
        res.set("FileDescription", "Librarian — music project browser");
        res.compile().expect("couldn't embed the Windows icon");
    }
}
