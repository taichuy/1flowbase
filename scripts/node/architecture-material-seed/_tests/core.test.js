const test = require("node:test");
const assert = require("node:assert/strict");

const manifest = require("../manifest.json");
const { blockDescriptor, blockSource, ensurePage } = require("../core.js");

function jsonResponse(status, body) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function fakeClient(responses) {
  const requests = [];
  return {
    requests,
    client: {
      apiBaseUrl: "http://api.test",
      cookie: "session=test",
      csrfToken: "csrf-test",
      async fetchImpl(url, init) {
        requests.push({ url: String(url), init });
        const next = responses.shift();
        assert.ok(next, `unexpected request: ${init.method} ${url}`);
        return jsonResponse(next.status, next.body);
      },
    },
  };
}

test("AC-010 create-new unpacks the Frontstage page creation envelope", async () => {
  const page = {
    id: "page-created",
    kind: "page",
    slug: manifest.targets.page_slug,
    children: [],
  };
  const defaultTab = { id: "tab-created", is_default: true };
  const fake = fakeClient([
    { status: 200, body: { data: [] } },
    { status: 201, body: { data: { page, default_tab: defaultTab } } },
  ]);

  const result = await ensurePage(fake.client, "workspace-1");

  assert.deepEqual(result, { page, defaultTab });
  assert.equal(fake.requests[1].init.method, "POST");
  assert.deepEqual(JSON.parse(fake.requests[1].init.body), {
    title: "资料库",
    placement: "topbar",
    slug: manifest.targets.page_slug,
  });
});

test("AC-010 resume-existing reuses the page and its backend default tab", async () => {
  const page = {
    id: "page-existing",
    kind: "page",
    slug: manifest.targets.page_slug,
    children: [],
  };
  const defaultTab = { id: "tab-existing", is_default: true };
  const fake = fakeClient([
    { status: 200, body: { data: [page] } },
    {
      status: 200,
      body: { data: [{ id: "tab-other", is_default: false }, defaultTab] },
    },
  ]);

  const result = await ensurePage(fake.client, "workspace-1");

  assert.deepEqual(result, { page, defaultTab });
  assert.equal(fake.requests.length, 2);
  assert.match(fake.requests[1].url, /pages\/page-existing\/tabs$/u);
});

test("AC-010 public Block resources do not leak a data model code or runtime route", () => {
  const node = {
    kind: "section",
    sourceKey: "section-01-01",
    title: "1.1 系统架构概述",
    content: "## 1.1 系统架构概述\n正文",
  };
  const descriptor = blockDescriptor(node);
  const source = blockSource(node);
  const publicMaterial = `${JSON.stringify(descriptor)}\n${source}`;

  assert.equal(descriptor.sourceKey, node.sourceKey);
  assert.match(source, /系统架构概述/u);
  assert.doesNotMatch(publicMaterial, /dataModelCode|model_code/u);
  assert.doesNotMatch(publicMaterial, /architecture_teaching_material_nodes/u);
  assert.doesNotMatch(publicMaterial, /\/api\/runtime\/models\//u);
});
