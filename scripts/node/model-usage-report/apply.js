// Explicit one-page rollout. Backups retain the exact pre-change authoring records.
const fs = require("node:fs");
const path = require("node:path");
const {
  loadRootCredentials,
  openTemporaryOwnerSession,
} = require("../page-debug/auth");
const { buildWorkflow } = require("./workflow");
const { retainedBlockPatch } = require("./retained-block");
const PAGE = "01a073f3-03e9-7030-86a4-371f80ebf522";
const BLOCK = "01a07465-8419-7eb2-9335-a193bffb7558";
const APP = "01a07410-8122-7371-a6af-2e5b60466379";
const titles = ["模型用量总览", "Token 消耗趋势", "用户 Token 消耗和费用"];
const rangeSchema = {
  type: "object",
  required: ["preset", "started_from", "started_to"],
  properties: {
    preset: { type: "string", enum: ["24h", "7d", "30d", "custom"] },
    started_from: { type: "string" },
    started_to: { type: "string" },
  },
};
function source(file) {
  return (
    fs.readFileSync(path.join(__dirname, "block-common.jsx"), "utf8") +
    "\n" +
    fs.readFileSync(path.join(__dirname, file + ".jsx"), "utf8")
  );
}
async function withApi(run) {
  const repoRoot =
    process.env.USAGE_REPO_ROOT || path.resolve(__dirname, "../../..");
  const base = process.env.USAGE_API_BASE || "http://127.0.0.1:7800";
  const session = await openTemporaryOwnerSession({
    apiBaseUrl: base,
    ...loadRootCredentials({ repoRoot }),
  });
  const api = async (url, method = "GET", body) => {
    const response = await fetch(base + url, {
      method,
      headers: {
        cookie: session.cookie,
        "x-csrf-token": session.csrfToken,
        "content-type": "application/json",
      },
      ...(body ? { body: JSON.stringify(body) } : {}),
    });
    const text = await response.text();
    if (!response.ok)
      throw new Error(
        method + " " + url + ": " + response.status + " " + text.slice(0, 1200),
      );
    const data = text ? JSON.parse(text) : null;
    return data?.data ?? data;
  };
  try {
    return await run(api);
  } finally {
    await session.dispose();
  }
}
async function apply(mode) {
  const dir = path.resolve(
    process.env.USAGE_EVIDENCE_DIR || "tmp/test-governance/usage-report",
    new Date().toISOString().replaceAll(":", "-"),
  );
  fs.mkdirSync(dir, { recursive: true });
  const save = (name, value) =>
    fs.writeFileSync(
      path.join(dir, name + ".json"),
      JSON.stringify(value, null, 2),
    );
  await withApi(async (api) => {
    if (mode === "workflow") {
      const old = await api(`/api/console/applications/${APP}/orchestration`);
      const mapping = await api(`/api/console/applications/${APP}/api-mapping`);
      save("original-workflow", old);
      save("original-mapping", mapping);
      const document = buildWorkflow(old.draft.document);
      save("workflow", document);
      await api(`/api/console/applications/${APP}/orchestration/draft`, "PUT", {
        change_kind: "logical",
        document,
        summary: "统一报表时间范围与Token、趋势、用户费用聚合",
      });
      save(
        "publication",
        await api(`/api/console/applications/${APP}/api-publications`, "POST", {
          api_enabled: true,
          mapping,
        }),
      );
    } else if (mode === "page") {
      const root = `/api/console/frontstage/pages/${PAGE}/blocks`;
      const original = await api(root + "/" + BLOCK);
      save("original-block", original);
      save("original-code", await api(root + "/" + BLOCK + "/code"));
      const roots = await api(
        root + "?tab_id=" + original.tab_id + "&limit=100",
      );
      save("original-roots", roots);
      const items = Array.isArray(roots) ? roots : roots.items;
      const ids = [];
      for (const [i, file] of ["overview", "trend", "users"].entries()) {
        const current =
          i === 0
            ? original
            : items.find(
                (item) => item.description === "model-usage-report:" + file,
              );
        const descriptor = structuredClone(original.runtime_descriptor);
        delete descriptor.id;
        delete descriptor.codeRef;
        descriptor.ports = {
          inputs: [{ name: "timeRange", schema: rangeSchema }],
          outputs: [{ name: "timeRange", schema: rangeSchema }],
        };
        descriptor["x-layout"] = { ...descriptor["x-layout"], order: i };
        const body = {
          title: titles[i],
          description: "model-usage-report:" + file,
          presentation: "inline",
          input_mapping: { timeRange: "usage.timeRange" },
          output_mapping: { timeRange: "usage.timeRange" },
          runtime_descriptor: descriptor,
        };
        Object.assign(body, retainedBlockPatch(body, i === 0));
        if (current) {
          if (i !== 0) {
            save("original-" + file, current);
            save(
              "original-" + file + "-code",
              await api(root + "/" + current.block_id + "/code"),
            );
          }
          await api(root + "/" + current.block_id, "PATCH", body);
          const previous = await api(root + "/" + current.block_id + "/code");
          await api(root + "/" + current.block_id + "/code", "PUT", {
            source_code: source(file),
            expected_source_revision: previous.source_sha256,
          });
          ids.push(current.block_id);
        } else {
          const created = await api(root, "POST", {
            ...body,
            tab_id: original.tab_id,
            parent_block_id: null,
            before_block_id: null,
            after_block_id: ids.at(-1),
            source_code: source(file),
          });
          ids.push(created.block_id);
        }
      }
      save("block-ids", ids);
      const components = await api(
        "/api/console/settings/ui-management/components",
      );
      const component = {
        component_code: "token-usage-trend",
        name: "Token 消耗趋势",
        description:
          "输入、输出、缓存Token与命中率双轴趋势；应用统计和低代码共享。",
        import_code:
          "import { TokenTrendChart } from '@1flowbase/token-trend';",
        source_code: "<TokenTrendChart points={points} />",
        source: "workspace",
        group: "reports",
        upstream: { identity: "1flowbase/token-trend", version: "1.0.0" },
        version: "1.0.0",
        keywords: ["tokens", "费用", "趋势"],
      };
      const existing = (
        Array.isArray(components) ? components : components.items
      ).find((c) => c.component_code === component.component_code);
      if (!existing)
        save(
          "component",
          await api(
            "/api/console/settings/ui-management/components",
            "POST",
            component,
          ),
        );
      console.log(JSON.stringify({ page_id: PAGE, block_ids: ids }));
    } else throw new Error("Usage: node apply.js workflow|page");
  });
  console.log("Evidence: " + dir);
}
if (require.main === module)
  apply(process.argv[2]).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
module.exports = { withApi, source, rangeSchema, PAGE, BLOCK, APP };
