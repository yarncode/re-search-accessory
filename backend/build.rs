fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");

    // Explorer/taskbar icon + version strings for the windows_subsystem binary.
    // `winresource` shells out to windres from the MinGW toolchain.
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();

        res.set_icon("assets/app.ico");
        res.set("ProductName", "re-search-accessory");
        res.set("FileDescription", "re-search-accessory — tra cứu linh kiện & BOM");
        res.set("LegalCopyright", "Dữ liệu công khai của thegioiic.com");

        if let Err(err) = res.compile() {
            // An icon is cosmetic: never fail the build because of it.
            println!("cargo:warning=cannot embed the Windows icon: {err}");
        }
    }
}
