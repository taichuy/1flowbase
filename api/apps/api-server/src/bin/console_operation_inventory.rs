use std::{env, fs, path::PathBuf};

use anyhow::{bail, Context, Result};
use api_server::app_state::compile_core_console_operation_inventory_snapshot;

fn main() -> Result<()> {
    let mut arguments = env::args_os().skip(1);
    let Some(output_path) = arguments.next() else {
        bail!("usage: console_operation_inventory <output-path>");
    };
    if arguments.next().is_some() {
        bail!("console_operation_inventory accepts exactly one output path");
    }

    let output_path = PathBuf::from(output_path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create inventory directory {}", parent.display())
        })?;
    }
    let snapshot = compile_core_console_operation_inventory_snapshot()?;
    let serialized = serde_json::to_string_pretty(&snapshot)?;
    fs::write(&output_path, format!("{serialized}\n")).with_context(|| {
        format!(
            "failed to write compiled console inventory {}",
            output_path.display()
        )
    })?;
    println!("{}", output_path.display());
    Ok(())
}
