use anyhow::{ensure, Result};
use serde_json::Value;

use crate::application_public_api::compat::openai::history::prove_full_context_input;

/// Assert the identical-retry subset of the production continuation proof.
pub(crate) fn validate_full_retry_input(
    input: &Value,
    trusted_history: &Value,
    owned_call_ids: &[String],
) -> Result<()> {
    let proof = prove_full_context_input(input, trusted_history, owned_call_ids)?;
    ensure!(
        proof.context.is_empty(),
        "native_history_item_count_mismatch"
    );
    Ok(())
}
