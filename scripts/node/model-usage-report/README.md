# Model Usage Report

Scoped authoring rollout for page `01a073f3-03e9-7030-86a4-371f80ebf522` and its existing reporting workflow. Issue #2223 owns acceptance.

- `report.sql`: one filtered log set for totals, Shanghai time buckets and users. Request count includes every logged request; absent usage remains null, empty buckets are zero, fees retain their currencies. Cache rate is the input-volume-weighted recorded provider rate, not a second token total.
- `range.js`: validates and normalizes paired timestamps before templating SQL; defaults to rolling 24 hours, selects hour/day buckets.
- `workflow.js`: upgrades the existing draft without changing its output extraction or endpoint.
- JSX sources: three blocks; shared input/output mapping `timeRange → usage.timeRange`. Only the overview initializes the value. User events publish it; incoming changes do not republish. Each request ignores completion after its effect is superseded.
- `apply.js`: authenticated authoring APIs, source revision checks, exact backups and temporary session cleanup. Run workflow then page only with the corresponding frontend candidate available. The database scope is intentionally the existing page's workspace, not caller-supplied SQL.

```sh
node scripts/node/model-usage-report/apply.js workflow
node scripts/node/model-usage-report/apply.js page
node --test scripts/node/model-usage-report/_tests/*.test.js
```

`USAGE_REPO_ROOT` selects the local credentials/environment owner when using an isolated checkout; `USAGE_API_BASE` defaults to local port 7800. Evidence/backups default to `tmp/test-governance/usage-report/`. No request-log facts are modified. SQL fixtures use a transaction-local temporary table and rollback. To undo configuration, restore the recorded original workflow draft/mapping and block records through the same authoring APIs, then republish the prior workflow. Do not overwrite later user edits.
