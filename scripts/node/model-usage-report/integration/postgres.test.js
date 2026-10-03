const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { main } = require("../range");

const databaseUrl = process.env.API_DATABASE_URL || process.env.DATABASE_URL;
if (!databaseUrl) throw new Error("PostgreSQL evidence requires API_DATABASE_URL or DATABASE_URL");
const database = new URL(databaseUrl);
if (!['postgres:', 'postgresql:'].includes(database.protocol)) {
  throw new Error("PostgreSQL evidence requires a PostgreSQL connection URL");
}
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
      decodeURIComponent(database.pathname.slice(1)),
      "-At",
      "-v",
      "ON_ERROR_STOP=1",
    ],
    {
      input: query,
      env: {
        ...process.env,
        PGPASSWORD: database.password
          ? decodeURIComponent(database.password)
          : process.env.PGPASSWORD,
        ...(database.searchParams.has('sslmode')
          ? { PGSSLMODE: database.searchParams.get('sslmode') }
          : {}),
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
function runFixture(reportSource) {
  const query = `BEGIN;
 CREATE TEMP TABLE model_provider_request_logs(scope_id uuid,user_id uuid,user_account text,currency_code text,total_cost numeric,started_at timestamptz,input_tokens bigint,output_tokens bigint,total_tokens bigint,input_cache_hit_tokens bigint,cache_write_tokens bigint,input_cache_hit_rate double precision);
 INSERT INTO model_provider_request_logs VALUES
 ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002','alice','USD',1,'2026-09-01T00:00Z',100,10,110,50,5,0.5),
 ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002','alice','CNY',2,'2026-09-01T01:00Z',200,20,220,100,0,0.5),
 ('00000000-0000-0000-0000-000000000001',NULL,NULL,NULL,NULL,'2026-09-01T02:00Z',NULL,NULL,NULL,NULL,NULL,NULL),
 ('00000000-0000-0000-0000-000000000001',NULL,NULL,'USD',99,'2026-09-02T00:00Z',999,0,999,0,0,0),
 ('00000000-0000-0000-0000-000000000009',NULL,NULL,'USD',99,'2026-09-01T00:00Z',999,0,999,0,0,0);
 ${reportSource}
 ROLLBACK;`;
  return JSON.parse(sql(query));
}

function assertReport(r) {
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
}

test("SQL groups users across models, keeps currencies and unknown usage, excludes end boundary and other scope", () => {
  assertReport(runFixture(reportSql("2026-09-01T00:00Z", "2026-09-02T00:00Z")));
});

test("SQL fixture rejects scope leakage and inclusive end boundary regressions", () => {
  const source = reportSql("2026-09-01T00:00Z", "2026-09-02T00:00Z");
  for (const [name, before, after] of [
    ["scope leakage", "l.scope_id = '00000000-0000-0000-0000-000000000001'::uuid", "true"],
    ["inclusive end boundary", "l.started_at < b.started_to", "l.started_at <= b.started_to"],
  ]) {
    assert.ok(source.includes(before), `fixture mutation anchor missing: ${name}`);
    const report = runFixture(source.replace(before, after));
    assert.throws(() => assertReport(report), assert.AssertionError, name);
  }
});
