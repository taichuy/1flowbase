use std::process::Command;

fn main() {
    let revision = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_owned());
    println!(
        "cargo:rustc-env=API_SERVER_BUILD_IDENTITY=v{}.git.{revision}",
        env!("CARGO_PKG_VERSION")
    );
}
