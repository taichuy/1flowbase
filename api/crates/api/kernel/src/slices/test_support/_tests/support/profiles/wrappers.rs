use super::sample_runtime_profile;
use runtime_profile::RuntimeProfile;
pub fn sample_api_profile(host_fingerprint: &str) -> RuntimeProfile {
    sample_runtime_profile("api-server", host_fingerprint)
}
pub fn sample_runner_profile(host_fingerprint: &str) -> RuntimeProfile {
    sample_runtime_profile("runtime-extension-host", host_fingerprint)
}
