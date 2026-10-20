# Capacity Report

`capacity-report` is the minimal control-plane inspection report for table capacity planning. It combines schema hygiene, high-growth table checks, raw JSONB boundaries, log query contract summary, and optional PostgreSQL size metrics into stable artifacts:

- `tmp/test-governance/capacity-report.json`
- `tmp/test-governance/capacity-report.md`

Default usage does not connect to a database:

```bash
node scripts/node/tooling.js capacity-report
```

Use an offline inspection payload when CI or a local smoke test already collected size metrics:

```bash
node scripts/node/tooling.js capacity-report --inspection-input tmp/test-governance/postgres-capacity-inspection.json
```

Use live PostgreSQL inspection only when explicitly requested:

```bash
node scripts/node/tooling.js capacity-report --database-url "$DATABASE_URL" --schema public
```

## Metadata Fields

| Field | Owner | Source of truth | Persisted | User editable | Historical impact | Evidence source |
| --- | --- | --- | --- | --- | --- | --- |
| `table_name` | schema hygiene | scan | no | no | identifies the table row in the report | migration inventory |
| `table_profile` | schema hygiene | scan | no | no | affects which hygiene rules apply | `schema-hygiene` table profile |
| `exemption_reason` | schema hygiene config | manual_reason | no | yes | may suppress a bounded hygiene finding until review | `schema-hygiene` exemption config |
| `growth_risk` | growth table report | scan | no | no | marks routing, uniqueness, index, and backfill work before scale planning | `growth-table-report` |
| `jsonb_risk` | raw JSONB report | scan | no | no | marks raw payload list-read risks before widening list APIs | `raw-jsonb-report` |
| `retention_archive_state` | capacity report config | manual_reason | no | yes | records whether retention/archive policy has been declared for future capacity work | `capacity-report` config |
| `total_size_bytes` | PostgreSQL inspection | live_postgres | no | no | point-in-time table plus index size | `pg_total_relation_size` |
| `table_size_bytes` | PostgreSQL inspection | live_postgres | no | no | point-in-time heap/table size | `pg_relation_size` |
| `index_size_bytes` | PostgreSQL inspection | live_postgres | no | no | point-in-time derived index size | total minus table size |
| `row_estimate` | PostgreSQL inspection | live_postgres | no | no | planner estimate used only as a planning signal | `pg_class.reltuples` |
| `collected_at` | PostgreSQL inspection | live_postgres | no | no | timestamp for interpreting point-in-time metrics | database clock |

## PostgreSQL Inspection Payload

Offline input must be JSON with a `metrics` array, or a raw array of metric objects:

```json
{
  "version": "postgres-capacity-inspection/v1",
  "metrics": [
    {
      "schema_name": "public",
      "table_name": "runtime_events",
      "total_size_bytes": 12582912,
      "table_size_bytes": 4194304,
      "index_size_bytes": 8388608,
      "row_estimate": 25000000,
      "collected_at": "2026-06-20T00:00:00.000Z"
    }
  ]
}
```

Live inspection reports clear error kinds:

- `connection_failure`
- `permission_denied`
- `statistics_unavailable`
- `postgres_inspection_failed`

When no inspection source is supplied, the report status is `skipped` for capacity metrics and the repository gate still writes the aggregate report.

## Assessment And Aggregate Status

`capacity-report/v1` keeps the existing table details, `growthRisk.findings` priorities,
`summary.growthFindings`, `partitionCandidates`, and `indexRisks` counts. Growth
`must_fix` / `later` means future topology readiness. An ordinary unpartitioned
primary key or a missing recommended index is not evidence of a current schema
or query defect. Planning details are retained in JSON and a separate Markdown
section, rather than discarded to obtain a clean aggregate status.

The additive `assessments` object separates the evidence:

| Field | Status | Meaning |
| --- | --- | --- |
| `currentContracts` | `passed` / `warning` | Current schema hygiene, raw JSONB list-read and log query contract findings. |
| `growthPlanning` | `planning` / `none` | Future routing, unique constraint, raw storage review and recommended index work; the original finding count and priorities remain available. |
| `measuredCapacity` | `unverified` / `observed` / `warning` | Metric coverage and measured threshold observations; this is not a query plan or performance verdict. |

`measuredCapacity` includes `coveredTables`, `totalTables`, `pressureTables`, and
`observations`. Each observation retains its table, signal, measured value,
threshold, source and collection time. A complete metric row includes finite
table/index/total sizes, a row estimate, and a collection timestamp. No metrics,
partial coverage, incomplete rows or inspection errors leave this assessment
`unverified` (未验证), even if a measured subset already shows pressure. A complete
snapshot without threshold pressure is `observed`, not capacity acceptance;
statistics freshness, representative data distribution and query plans still
need their own evidence.

The aggregate `status` continues to use `passed`, `warning`, and `failed`:

- `failed`: inspection failed, such as a connection or permission failure; exit code remains 1.
- `warning`: a current contract violation, measured threshold pressure, or inspection diagnostic exists; exit code remains 0, as before.
- `passed`: no current detectable contract violation, measured pressure or inspection error was found. This does **not** certify capacity. Planning alone does not promote this status to warning.

For example, 219 tables with no metrics and 75 growth planning findings retain
all 75 findings and their original candidate/index counts. Their current
contracts may be `passed`, while measured capacity is explicitly `unverified`
with coverage `0/219`. CLI and Markdown state this distinction. The legacy
candidate/index flags combine planning and observed signals; use
`assessments.measuredCapacity.observations` to identify measured pressure.

Actual metric thresholds still produce warning observations independently of
growth-table configuration. A large row estimate, total size or index/table
ratio does not prove that partitioning or a new index is required. Physical
partitioning, primary key semantics and index changes require a separate
architecture/query-plan decision. Current raw/schema/log violations are never
reclassified as growth planning.

Source: [Database, Query, And Ephemeral Audit](../../../.agents/skills/qa-evaluation/references/audit/database-query-ephemeral.md),
especially the legal planning negatives and unverified evidence boundary; see
also [Growth Table Planning Report](../growth-table-report/README.md).
