# Scope ID Routing Semantics

Use this reference when QA scope touches `scope_id`, `SYSTEM_SCOPE_ID`, owner-chain
rules, schema readiness, migrations, runtime physical tables, or report findings that
classify tables as `no_action`, `declare_generation_rule`, or `needs_owner_review`.

## Human Rules

- `scope_id` only means physical routing scope. It does not carry permission semantics.
- `workspace-owned`: `scope_id = workspace_id`.
- `application-owned`: `application_id -> applications.scope_id`.
- `flow-run child`: `flow_run_id -> flow_runs.scope_id`.
- `join / child`: inherit from a clear parent / owner. Do not decide scope locally.
- `system/global`: use explicit `SYSTEM_SCOPE_ID` after system/global semantics are declared.
- `mixed owner`: require `scope_kind`, owner discriminator, or owner chain before migration.
- `unknown`: stop at `needs_owner_review`.

## QA Checks

- Do not accept `workspace_id` as a readiness substitute for `scope_id`.
- Do not accept `scope_id` just because a column exists; require owner-chain, backfill,
  and write-path evidence or an explicit machine-readable declaration.
- Do not accept fixed or random database defaults for `scope_id`.
- Do not allow system/global rows to be silently routed into a workspace scope.
- For mixed owner tables, migration is not acceptable without a discriminator or owner chain.
- For new managed tables, missing `scopeGenerationSource`, `backfillSource`,
  `writePathSource`, or a clear `needsOwnerReviewTables` reason must remain visible as
  warning/error evidence.

## Expected Rule Location

- Human semantics live in the QA skill reference and the accepted ADR issue.
- Machine-readable enforcement lives in `scripts/node/schema-hygiene/config.json`.
- Report evidence should come from `schema-hygiene`, `growth-table-report`, migration
  smoke, or focused repository/write-path tests depending on the task scope.
