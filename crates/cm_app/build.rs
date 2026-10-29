//! Windows only: embed the app icon and version info in campaign-manager.exe.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set("ProductName", "Campaign Manager");
    res.set("FileDescription", "Campaign Manager — a desktop tool for Dungeon Masters");
    // A missing resource compiler (rc.exe / windres) only costs the icon, not the build.
    if let Err(e) = res.compile() {
        println!("cargo:warning=couldn't embed the Windows icon: {e}");
    }
}
