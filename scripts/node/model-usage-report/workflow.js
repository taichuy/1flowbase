const fs = require("node:fs");
const path = require("node:path");
function buildWorkflow(document) {
  const next = structuredClone(document);
  const nodes = next.graph.nodes;
  const start = nodes.find((n) => n.type === "workflow_start");
  start.config.input_fields = ["started_from", "started_to"].map((key) => ({
    key,
    label: key,
    inputType: "text",
    valueType: "string",
    source: "query",
    required: false,
  }));
  const range = {
    id: "node-report-range",
    type: "code",
    alias: "校验统计时间范围",
    configVersion: 1,
    config: {
      source: fs
        .readFileSync(path.join(__dirname, "range.js"), "utf8")
        .split("if (typeof module")[0],
      language: "javascript",
    },
    bindings: Object.fromEntries(
      ["started_from", "started_to"].map((key) => [
        key,
        { kind: "selector", value: [start.id, key] },
      ]),
    ),
    outputs: ["started_from", "started_to", "bucket"].map((key) => ({
      key,
      title: key,
      valueType: "string",
    })),
    position: { x: 350, y: 220 },
    containerId: null,
    description: "默认24小时，校验并规范化时间，禁止原始参数进入SQL。",
  };
  next.graph.nodes = nodes.filter((n) => n.id !== range.id);
  next.graph.nodes.splice(1, 0, range);
  const sql = next.graph.nodes.find((n) => n.id === "node-usage-sql");
  sql.bindings.sql = {
    kind: "templated_text",
    value: fs.readFileSync(path.join(__dirname, "report.sql"), "utf8"),
  };
  sql.description =
    "当前工作区请求日志：统一时间范围、Token分项、趋势和用户费用；币种分开。";
  next.graph.edges = next.graph.edges.filter((e) => e.id !== "edge-range-sql");
  next.graph.edges.find((e) => e.source === start.id).target = range.id;
  next.graph.edges.push({
    id: "edge-range-sql",
    source: range.id,
    target: sql.id,
    points: [],
    sourceHandle: null,
    targetHandle: null,
    containerId: null,
  });
  return next;
}
module.exports = { buildWorkflow };
