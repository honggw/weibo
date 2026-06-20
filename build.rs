fn main() {
    #[cfg(target_os = "windows")]
    {
        // webview2-com-sys v0.36 requires these system libraries
        println!("cargo:rustc-link-lib=advapi32");
    }
}
