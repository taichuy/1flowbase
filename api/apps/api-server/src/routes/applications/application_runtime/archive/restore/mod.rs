use super::*;

mod records;
mod run_transaction;

#[derive(Default)]
struct ArchiveRestoreIdMaps {
    node_runs: std::collections::HashMap<Uuid, Uuid>,
    runtime_spans: std::collections::HashMap<Uuid, Uuid>,
    runtime_events: std::collections::HashMap<Uuid, Uuid>,
    runtime_items: std::collections::HashMap<Uuid, Uuid>,
    usage_ledger: std::collections::HashMap<Uuid, Uuid>,
    model_failover_attempts: std::collections::HashMap<Uuid, Uuid>,
    context_projections: std::collections::HashMap<Uuid, Uuid>,
}

use records::*;
pub(crate) use run_transaction::restore_run_archive_v1;
