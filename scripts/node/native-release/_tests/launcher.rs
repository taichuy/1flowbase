// Test adapter for the real launch scripts: replace only release lookup with a known local package.
// The actual updater still performs preflight, replacement, startup validation and cleanup.
fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    assert_eq!(arguments.len(), 3);
    assert_eq!(arguments[1], "--upgrade");
    let target = std::path::PathBuf::from(&arguments[2]).canonicalize().unwrap();
    assert!(!std::env::current_exe().unwrap().canonicalize().unwrap().starts_with(&target), "launcher must run outside the installation");
    let status = std::process::Command::new(std::env::var_os("FLOWBASE_TEST_UPDATER").unwrap())
        .arg("--apply-update")
        .arg(std::env::var_os("FLOWBASE_TEST_UPDATE_PACKAGE").unwrap())
        .arg(target)
        .status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
