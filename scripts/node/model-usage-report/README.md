# Model Usage Report

Scoped authoring rollout for page `01a073f3-03e9-7030-86a4-371f80ebf522` and its existing reporting workflow. Issue #2223 owns acceptance.

- `report.sql`: one filtered log set for totals, Shanghai time buckets and users. Request count includes every logged request; absent usage remains null, empty buckets are zero, fees retain their currencies. Cache rate is the input-volume-weighted recorded provider rate, not a second token total.
- `range.js`: validates and normalizes paired timestamps before templating SQL; defaults to rolling 24 hours, selects hour/day buckets.
- `workflow.js`: upgrades the existing draft without changing its output extraction or endpoint.
- JSX sources: three blocks; shared input/output mapping `timeRange → usage.timeRange`. Only the compact overview renders the time filter and initializes the value. Its filter events publish the shared range. The overview is the only authorized request producer and publishes `usage.reportState`; trend and user table consume the retained snapshot. A retained in-flight request survives Activity subscription cleanup, and superseded responses cannot replace a newer range. Pending/error states keep the previous report and its original time interval. Only the current request and visible snapshot are retained; no query history or report data is persisted to browser storage.
- `apply.js`: authenticated authoring APIs, source revision checks, exact backups and temporary session cleanup. Run workflow then page only with the corresponding frontend candidate available. The database scope is intentionally the existing page's workspace, not caller-supplied SQL.

```sh
node scripts/node/model-usage-report/apply.js workflow
node scripts/node/model-usage-report/apply.js page
# Repository tests: pure range/workflow behavior, no service or private .env.
node --test scripts/node/model-usage-report/_tests/*.test.js
```

For authoring with `apply.js`, `USAGE_REPO_ROOT` selects the local credentials/environment owner when using an isolated checkout; `USAGE_API_BASE` defaults to local port 7800. Evidence/backups default to `tmp/test-governance/usage-report/`. No request-log facts are modified. To undo configuration, restore the recorded original workflow draft/mapping and block records through the same authoring APIs, then republish the prior workflow. Do not overwrite later user edits.

PostgreSQL algorithm evidence has a separate service lane:

```sh
node --test scripts/node/model-usage-report/integration/postgres.test.js
```

This requires `psql` and an explicit `API_DATABASE_URL` or `DATABASE_URL` (in that order). It never reads a private `.env`; missing configuration fails. The complete SQL algorithm executes against transaction-local temporary request-log rows and rolls back. Assertions retain currency separation, unknown usage, user aggregation, Shanghai buckets, exclusive end boundary and scope isolation. Controlled SQL variants remove the scope filter or include the end boundary; the fixture must reject both. No API deployment or credentials are needed for this lane.

Published-report deployment acceptance is separate from the repository and PostgreSQL gates:

```sh
node --test scripts/node/model-usage-report/acceptance/published-report.test.js
```

Provide the database URL above plus all of these environment variables; none has a deployment default:

- `USAGE_API_BASE`, `USAGE_REPORT_PATH`: HTTP(S) API base and published report path without query or fragment.
- `USAGE_ROOT_ACCOUNT`, `USAGE_ROOT_PASSWORD`: account for a temporary owner session; the shared `page-debug` session owner revokes it in `finally`.
- `USAGE_SCOPE_ID`, `USAGE_PAGE_ID`, `USAGE_TAB_ID`, `USAGE_BLOCK_ID`: UUIDs of the matching deployed workspace and callable report context.
- `USAGE_STARTED_FROM`, `USAGE_STARTED_TO`: explicit interval, normalized and validated by the actual report range function.

The acceptance suite compares the published response to independent request-log SQL totals and exercises the configured frontstage callable gateway. It does not create or publish a report fixture. The selected API deployment, database, scope and callable context must refer to the same report. Repository/SQL gate success alone does not verify a published deployment, and deployment acceptance must be recorded separately with its candidate and configuration context (without credentials).

Issue #2252 adds the scoped retention rollout: `node scripts/node/model-usage-report/apply-retention.js`. It changes only the three existing blocks' source and signal mappings, preserves their layout/title/other configuration, backs up exact records, and rejects unknown source revisions. Use it instead of the full `apply.js page` rollout for this repair. `originals-and-plan.json` stores the original source/descriptor/mappings and `applied.json` stores the installed hashes; rollback must verify those hashes before restoring through authoring APIs.
