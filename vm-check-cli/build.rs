fn main() {
    // build.rs itself always runs as a *host* binary, so `cfg(target_os)`
    // here would reflect the machine doing the compiling, not the platform
    // being built for. `CARGO_CFG_TARGET_OS` is the env var Cargo sets to the
    // actual target OS, which is what matters when cross-compiling for
    // Windows from Linux. `winresource` is a plain build-dependency (not
    // gated in Cargo.toml) precisely so it's always available here to
    // compile, on any host, and simply does nothing unless asked to.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile()
            .expect("failed to embed Windows icon resource");
    }
}
