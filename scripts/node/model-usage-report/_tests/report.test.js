const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { parseEnvFile } = require("../../dev-up/env");
const { withApi } = require("../apply");
const { main } = require("../range");
const root =
  process.env.USAGE_REPO_ROOT || path.resolve(__dirname, "../../../..");
const database = new URL(
  parseEnvFile(path.join(root, "api/apps/api-server/.env")).API_DATABASE_URL,
);
function sql(query) {
  return execFileSync(
    "psql",
    [
      "-X",
      "-q",
      "-h",
      database.hostname,
      "-p",
      database.port || "5432",
      "-U",
      decodeURIComponent(database.username),
      "-d",
      database.pathname.slice(1),
      "-At",
      "-v",
      "ON_ERROR_STOP=1",
    ],
    {
      input: query,
      env: {
        ...process.env,
        PGPASSWORD: decodeURIComponent(database.password),
      },
      encoding: "utf8",
    },
  ).trim();
}
function reportSql(from, to) {
  const bounds = main({ started_from: from, started_to: to });
  return fs
    .readFileSync(path.join(__dirname, "../report.sql"), "utf8")
    .replace(
      /\{\{ node-report-range.result.(\w+) \}\}/g,
      (_, key) => bounds[key],
    );
}
test("SQL groups users across models, keeps currencies and unknown usage, excludes end boundary and other scope", () => {
  const query = `BEGIN;
 CREATE TEMP TABLE model_provider_request_logs(scope_id uuid,user_id uuid,user_account text,currency_code text,total_cost numeric,started_at timestamptz,input_tokens bigint,output_tokens bigint,total_tokens bigint,input_cache_hit_tokens bigint,cache_write_tokens bigint,input_cache_hit_rate double precision);
 INSERT INTO model_provider_request_logs VALUES
 ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002','alice','USD',1,'2026-09-01T00:00Z',100,10,110,50,5,0.5),
 ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002','alice','CNY',2,'2026-09-01T01:00Z',200,20,220,100,0,0.5),
 ('00000000-0000-0000-0000-000000000001',NULL,NULL,NULL,NULL,'2026-09-01T02:00Z',NULL,NULL,NULL,NULL,NULL,NULL),
 ('00000000-0000-0000-0000-000000000001',NULL,NULL,'USD',99,'2026-09-02T00:00Z',999,0,999,0,0,0),
 ('00000000-0000-0000-0000-000000000009',NULL,NULL,'USD',99,'2026-09-01T00:00Z',999,0,999,0,0,0);
 ${reportSql("2026-09-01T00:00Z", "2026-09-02T00:00Z")}
 ROLLBACK;`;
  const r = JSON.parse(sql(query));
  assert.equal(r.request_count, 3);
  assert.equal(r.total_tokens, 330);
  assert.equal(r.input_tokens, 300);
  assert.equal(r.output_tokens, 30);
  assert.equal(r.input_cache_hit_tokens, 150);
  assert.equal(r.cache_write_tokens, 5);
  assert.equal(r.usage_recorded_count, 2);
  assert.equal(r.unbilled_count, 1);
  assert.equal(r.users.length, 2);
  assert.deepEqual(
    r.costs
      .filter((c) => c.total_cost !== null)
      .map((c) => [c.currency_code, Number(c.total_cost)]),
    [
      ["CNY", 2],
      ["USD", 1],
    ],
  );
  assert.equal(r.trend.length, 24);
  assert.equal(
    r.trend.reduce((s, p) => s + p.request_count, 0),
    3,
  );
  assert.equal(r.trend[0].input_cache_hit_rate, 0.5);
  assert.equal(r.trend[2].total_tokens, null);
  assert.equal(r.trend[3].total_tokens, 0);
  assert.equal(r.trend[3].input_cache_hit_rate, null);
  assert.equal(r.users.find((u) => u.user_id === null).total_tokens, null);
});
test("published API matches independent request-log totals for fixed interval", async () => {
  const from = "2026-09-29T00:00:00Z",
    to = "2026-10-03T00:00:00Z";
  const expected = JSON.parse(
    sql(
      `select jsonb_build_object('request_count',count(*),'total_tokens',sum(total_tokens),'input_tokens',sum(input_tokens),'output_tokens',sum(output_tokens),'input_cache_hit_tokens',sum(input_cache_hit_tokens),'cache_write_tokens',sum(cache_write_tokens)) from public.model_provider_request_logs where scope_id='00000000-0000-0000-0000-000000000001' and started_at>='${from}' and started_at<'${to}';`,
    ),
  );
  await withApi(async (api) => {
    const { report: r } = await api(
      "/api/ex/model-usage-report?started_from=" + from + "&started_to=" + to,
    );
    for (const key of Object.keys(expected))
      assert.equal(r[key], expected[key], key);
    assert.equal(
      r.users.reduce((n, u) => n + u.request_count, 0),
      r.request_count,
    );
    assert.equal(
      r.trend.reduce((n, u) => n + u.request_count, 0),
      r.request_count,
    );
  });
});

test("frontstage callable gateway accepts structured query parameters and returns report", async () => {
  await withApi(async (api) => {
    const { report } = await api(
      "/api/console/frontstage/pages/01a073f3-03e9-7030-86a4-371f80ebf522/tabs/01a073f3-03e9-7030-86a4-372f129962fa/callable-interfaces/dispatch",
      "POST",
      {
        block_id: "01a07465-8419-7eb2-9335-a193bffb7558",
        method: "GET",
        path: "/api/ex/model-usage-report",
        request: {
          query: {
            started_from: "2026-09-29T00:00:00Z",
            started_to: "2026-10-03T00:00:00Z",
          },
        },
      },
    );
    assert.ok(report.request_count >= 0);
    assert.equal(
      Date.parse(report.started_from),
      Date.parse("2026-09-29T00:00:00Z"),
    );
    assert.equal(
      Date.parse(report.started_to),
      Date.parse("2026-10-03T00:00:00Z"),
    );
    assert.equal(
      report.trend.reduce((n, p) => n + p.request_count, 0),
      report.request_count,
    );
  });
});
