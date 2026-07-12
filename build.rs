//! Build script: embeds the application icon into the Windows executable.

fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/whirr.ico");
        if let Err(err) = res.compile() {
            println!("cargo:warning=failed to embed Windows resources: {err}");
        }
    }
}
