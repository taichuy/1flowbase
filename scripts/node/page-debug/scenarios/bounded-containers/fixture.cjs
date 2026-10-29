"use strict";
const assert = require("node:assert/strict");

const ids = {
  start: "bc-start",
  input: "bc-input",
  iteration: "bc-iteration",
  innerLoop: "bc-inner-loop",
  leaf: "bc-leaf",
  after: "bc-after",
  end: "bc-end",
};
const node = (id, type, alias, position, extra = {}) => ({
  id,
  type,
  alias,
  description: "",
  containerId: null,
  position,
  configVersion: 1,
  config: {},
  bindings: {},
  outputs: [],
  ...extra,
});
const edge = (source, target, containerId = null) => ({
  id: `edge-${source}-${target}`,
  source,
  target,
  sourceHandle: null,
  targetHandle: null,
  containerId,
  points: [],
});
const selector = (...value) => ({ kind: "selector", value });
const output = (key, valueType) => ({ key, title: key, valueType });

function initialDocument(document, name) {
  return {
    ...document,
    meta: { ...document.meta, name },
    graph: {
      nodes: [
        node(
          ids.start,
          "workflow_start",
          "BC Start",
          { x: 80, y: 180 },
          {
            config: {
              input_fields: [
                {
                  key: "items_json",
                  label: "Items JSON",
                  inputType: "paragraph",
                  valueType: "string",
                  required: true,
                  hidden: false,
                },
              ],
              sync_timeout_ms: 60000,
            },
          },
        ),
        node(
          ids.input,
          "code",
          "BC Input",
          { x: 360, y: 180 },
          {
            config: {
              language: "javascript",
              source:
                "function main({ items_json }) { return { items: JSON.parse(items_json) }; }",
            },
            bindings: {
              named_bindings: {
                kind: "named_bindings",
                value: [
                  {
                    name: "items_json",
                    value: {
                      kind: "selector",
                      selector: [ids.start, "items_json"],
                    },
                  },
                ],
              },
            },
            outputs: [
              { ...output("items", "array"), selector: ["result", "items"] },
            ],
          },
        ),
        node(ids.end, "workflow_end", "BC End", { x: 640, y: 180 }),
      ],
      edges: [edge(ids.start, ids.input), edge(ids.input, ids.end)],
    },
    editor: { ...document.editor, viewport: { x: 0, y: 0, zoom: 1 } },
  };
}

function nestedDocument(uiDocument, loopId) {
  // Keep the actual UI-created loop and its automatic start; do not fabricate that proof.
  const loop = uiDocument.graph.nodes.find((node) => node.id === loopId);
  const starts = uiDocument.graph.nodes.filter(
    (node) => node.containerId === loopId && node.type === "container_start",
  );
  assert.equal(
    starts.length,
    1,
    "UI insertion must persist one container_start",
  );
  assert.ok(loop && loop.type === "loop");
  const iterationStartId = `${ids.iteration}-start`;
  return {
    ...uiDocument,
    graph: {
      nodes: [
        ...uiDocument.graph.nodes.filter((node) =>
          [ids.start, ids.input].includes(node.id),
        ),
        {
          ...loop,
          alias: "BC Loop",
          position: { x: 640, y: 180 },
          width: 1200,
          height: 660,
          config: {
            ...loop.config,
            max_rounds: 3,
            round_timeout_ms: 20000,
            error_policy: "fail",
          },
          bindings: { output: selector(ids.iteration, "result") },
        },
        { ...starts[0], position: { x: 32, y: 88 } },
        node(
          ids.iteration,
          "iteration",
          "BC Iteration",
          { x: 156, y: 120 },
          {
            containerId: loopId,
            width: 1000,
            height: 500,
            config: {
              parallelism: 1,
              flatten_output: true,
              round_timeout_ms: 10000,
              error_policy: "fail",
            },
            // The scenario chooses items through the real Cascader after loading this preparation.
            bindings: { output: selector(ids.innerLoop, "result") },
            outputs: [output("result", "array")],
          },
        ),
        node(
          iterationStartId,
          "container_start",
          "BC Inner Start",
          { x: 32, y: 88 },
          { containerId: ids.iteration },
        ),
        node(
          ids.innerLoop,
          "loop",
          "BC Inner Loop",
          { x: 156, y: 120 },
          {
            containerId: ids.iteration,
            width: 680,
            height: 330,
            config: {
              max_rounds: 1,
              round_timeout_ms: 5000,
              error_policy: "fail",
              loop_variables: [],
            },
            bindings: { output: selector(ids.leaf, "text") },
            outputs: [output("result", "array"), output("variables", "object")],
          },
        ),
        node(
          `${ids.innerLoop}-start`,
          "container_start",
          "BC Deep Start",
          { x: 32, y: 88 },
          { containerId: ids.innerLoop },
        ),
        node(
          ids.leaf,
          "template_transform",
          "BC Item",
          { x: 170, y: 120 },
          {
            containerId: ids.innerLoop,
            bindings: {
              template: {
                kind: "templated_text",
                value: `{{${loopId}.index}}:{{${ids.iteration}.item}}`,
              },
            },
            outputs: [output("text", "string")],
          },
        ),
        node(
          ids.after,
          "template_transform",
          "BC After",
          { x: 1960, y: 180 },
          {
            bindings: { template: { kind: "templated_text", value: "done" } },
            outputs: [output("text", "string")],
          },
        ),
        node(
          ids.end,
          "workflow_end",
          "BC End",
          { x: 2240, y: 180 },
          {
            bindings: {
              results: selector(loopId, "result"),
              after: selector(ids.after, "text"),
            },
            outputs: [output("results", "array"), output("after", "string")],
          },
        ),
      ],
      edges: [
        edge(ids.start, ids.input),
        edge(ids.input, loopId),
        edge(loopId, ids.after),
        edge(ids.after, ids.end),
        edge(starts[0].id, ids.iteration, loopId),
        edge(iterationStartId, ids.innerLoop, ids.iteration),
        edge(`${ids.innerLoop}-start`, ids.leaf, ids.innerLoop),
      ],
    },
  };
}

function assertRun(detail, loopId) {
  assert.equal(detail.flow_run.status, "succeeded");
  assert.deepEqual(detail.flow_run.output_payload, {
    results: [
      ["0:1", "0:2", "0:3"],
      ["1:1", "1:2", "1:3"],
    ],
    after: "done",
  });
  const inputRuns = detail.node_runs.filter((run) => run.node_id === ids.input);
  assert.equal(inputRuns.length, 1);
  assert.deepEqual(inputRuns[0].output_payload.result.items, [1, 2, 3]);
  const leaves = detail.node_runs.filter((run) => run.node_id === ids.leaf);
  assert.equal(
    leaves.length,
    6,
    "each leaf execution must have its own persisted node_run",
  );
  assert.equal(new Set(leaves.map((run) => run.id)).size, 6);
  assert.equal(
    new Set(leaves.map((run) => run.execution_identity?.execution_id)).size,
    6,
  );
  const paths = leaves.map((run) => {
    assert.equal(run.status, "succeeded");
    const identity = run.execution_identity;
    assert.ok(
      identity,
      "typed execution_identity is required; debug text is not an identity source",
    );
    assert.ok(
      identity.execution_id,
      "logical execution identity is distinct from its run-scoped node receipt",
    );
    assert.equal(identity.node_id, ids.leaf);
    assert.ok(identity.parent_execution_id);
    assert.equal(identity.round_path.length, 3);
    const [outer, inner, deepest] = identity.round_path;
    assert.equal(outer.container_node_id, loopId);
    assert.equal(inner.container_node_id, ids.iteration);
    assert.equal(deepest.container_node_id, ids.innerLoop);
    assert.equal(deepest.round_index, 0);
    assert.ok(deepest.container_execution_id);
    assert.ok(outer.container_execution_id && inner.container_execution_id);
    assert.equal(
      run.output_payload.text,
      `${outer.round_index}:${inner.round_index + 1}`,
    );
    return `${outer.round_index}/${inner.round_index}/${deepest.round_index}`;
  });
  assert.deepEqual(paths.sort(), [
    "0/0/0",
    "0/1/0",
    "0/2/0",
    "1/0/0",
    "1/1/0",
    "1/2/0",
  ]);
  const after = detail.node_runs.filter((run) => run.node_id === ids.after);
  assert.equal(after.length, 1);
  assert.equal(after[0].status, "succeeded");
  assert.equal(after[0].output_payload.text, "done");
  return {
    status: detail.flow_run.status,
    output: detail.flow_run.output_payload,
    leaf_count: leaves.length,
    leaf_paths: paths,
    leaf_execution_ids: leaves.map(
      (run) => run.execution_identity.execution_id,
    ),
    leaf_node_run_ids: leaves.map((run) => run.id),
    successor_id: after[0].id,
  };
}
module.exports = { ids, initialDocument, nestedDocument, assertRun };
