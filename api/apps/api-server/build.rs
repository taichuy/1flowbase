fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Runtime::block_on polls startup and the health check on the main thread.
        // MSVC's default 1 MiB reserve overflows here, while Rust test threads pass.
        // Reserve 8 MiB for the API entry point; leave the initial commit unchanged.
        println!("cargo::rustc-link-arg-bin=api-server=/STACK:8388608");
    }
}
