// Gives the Windows program its icon and version details (shown in File Explorer).
// Does nothing when building for macOS or Linux.
fn main() {
    println!("cargo:rerun-if-changed=assets/Silvery.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/Silvery.ico")
        .set("ProductName", "Silvery")
        .set("FileDescription", "Silvery: private meeting notes with a local AI")
        .set("LegalCopyright", "MIT License, Silvery contributors");
    if let Err(e) = res.compile() {
        // A missing resource compiler shouldn't stop the build; the program just has no icon.
        println!("cargo:warning=Windows icon not added: {e}");
    }
}
