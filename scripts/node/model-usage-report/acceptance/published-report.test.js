const { test } = require("node:test");
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const { main } = require("../range");

const databaseUrl = process.env.API_DATABASE_URL || process.env.DATABASE_URL;
if (!databaseUrl) throw new Error("PostgreSQL evidence requires API_DATABASE_URL or DATABASE_URL");
const database = new URL(databaseUrl);
if (!['postgres:', 'postgresql:'].includes(database.protocol)) {
  throw new Error("PostgreSQL evidence requires a PostgreSQL connection URL");
}

const { openTemporaryOwnerSession } = require('../../page-debug/auth');
function required(name) {
  const value = process.env[name];
  if (!value) throw new Error(`Deployment acceptance requires ${name}`);
  return value;
}
function uuid(name) {
  const value = required(name);
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu.test(value)) {
    throw new Error(`Deployment acceptance requires a UUID for ${name}`);
  }
  return value;
}
const bounds = main({
  started_from: required('USAGE_STARTED_FROM'),
  started_to: required('USAGE_STARTED_TO'),
});
const config = {
  apiBase: required('USAGE_API_BASE').replace(/\/$/u, ''),
  account: required('USAGE_ROOT_ACCOUNT'),
  password: required('USAGE_ROOT_PASSWORD'),
  reportPath: required('USAGE_REPORT_PATH'),
  scopeId: uuid('USAGE_SCOPE_ID'),
  pageId: uuid('USAGE_PAGE_ID'),
  tabId: uuid('USAGE_TAB_ID'),
  blockId: uuid('USAGE_BLOCK_ID'),
  from: bounds.started_from,
  to: bounds.started_to,
};
if (!/^https?:$/u.test(new URL(config.apiBase).protocol)) {
  throw new Error('USAGE_API_BASE must be an HTTP(S) URL');
}
if (!config.reportPath.startsWith('/') || config.reportPath.startsWith('//') || /[?#]/u.test(config.reportPath)) {
  throw new Error('USAGE_REPORT_PATH must be an application path without query or fragment');
}
async function withApi(run) {
  const session = await openTemporaryOwnerSession({
    apiBaseUrl: config.apiBase,
    account: config.account,
    password: config.password,
  });
  try {
    return await run(async (url, method = 'GET', body) => {
      const response = await fetch(config.apiBase + url, {
        method,
        headers: {
          cookie: session.cookie,
          'x-csrf-token': session.csrfToken,
          'content-type': 'application/json',
        },
        ...(body ? { body: JSON.stringify(body) } : {}),
      });
      if (!response.ok) throw new Error(`${method} ${url}: ${response.status}`);
      const result = await response.json();
      return result?.data ?? result;
    });
  } finally {
    await session.dispose();
  }
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
test("published API matches independent request-log totals for configured interval", async () => {
  const { from, to } = config;
  const expected = JSON.parse(
    sql(
      `select jsonb_build_object('request_count',count(*),'total_tokens',sum(total_tokens),'input_tokens',sum(input_tokens),'output_tokens',sum(output_tokens),'input_cache_hit_tokens',sum(input_cache_hit_tokens),'cache_write_tokens',sum(cache_write_tokens)) from public.model_provider_request_logs where scope_id='${config.scopeId}' and started_at>='${from}' and started_at<'${to}';`,
    ),
  );
  await withApi(async (api) => {
    const { report: r } = await api(
      config.reportPath + "?" + new URLSearchParams({ started_from: from, started_to: to }),
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
      `/api/console/frontstage/pages/${config.pageId}/tabs/${config.tabId}/callable-interfaces/dispatch`,
      "POST",
      {
        block_id: config.blockId,
        method: "GET",
        path: config.reportPath,
        request: {
          query: {
            started_from: config.from,
            started_to: config.to,
          },
        },
      },
    );
    assert.ok(report.request_count >= 0);
    assert.equal(
      Date.parse(report.started_from),
      Date.parse(config.from),
    );
    assert.equal(
      Date.parse(report.started_to),
      Date.parse(config.to),
    );
    assert.equal(
      report.trend.reduce((n, p) => n + p.request_count, 0),
      report.request_count,
    );
  });
});
