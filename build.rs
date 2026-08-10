fn main() {
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        let mut res = winres::WindowsResource::new();
        let version = env!("CARGO_PKG_VERSION");
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "THide Hover + UWD2");
        res.set(
            "FileDescription",
            "Taskbar hover auto-hide with Insider watermark integration",
        );
        res.set("CompanyName", "Saulo Tassara");
        res.set(
            "LegalCopyright",
            "Copyright © 2026 Saulo Tassara; UWD2 integration under AGPL-3.0",
        );
        res.set("OriginalFilename", "thide.exe");
        res.set("InternalName", "thide-hover-uwd2");
        res.set("ProductVersion", version);
        res.set("FileVersion", version);
        res.set(
            "Comments",
            "THide Hover + UWD2 | Developer: Saulo Tassara | github.com/SauloTassara/thide-hover-uwd2 | based on github.com/amnweb/thide | UWD2: github.com/machineonamission/uwd2",
        );
        res.set("LegalTrademarks", "");

        res.compile().unwrap_or_else(|_| {
            eprintln!("Warning: Could not embed resources in executable");
        });
    }
}
