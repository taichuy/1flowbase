// Compile the exact production execution module on each native runner.
#[macro_use]
#[path = "../../../../crates/api/console-interaction/src/test_support_visibility.rs"]
mod test_support_visibility;

#[path = "../../../../crates/api/kernel/src/slices/local_command/process/mod.rs"]
pub mod process;

#[path = "../../../../crates/api/kernel/src/slices/local_command/jobs/mod.rs"]
pub mod jobs;

#[path = "../../../../crates/api/console-interaction/src/local_terminal/mod.rs"]
pub mod local_terminal;
