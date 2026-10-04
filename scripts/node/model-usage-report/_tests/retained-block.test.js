const { test } = require("node:test");
const assert = require("node:assert/strict");
const { retainedBlockPatch } = require("../retained-block");

test("retained report rollout preserves unrelated authoring configuration", () => {
  const block = {
    runtime_descriptor: {
      id: "existing",
      codeRef: "existing-code",
      "x-layout": { order: 9 },
      ports: {
        inputs: [{ name: "timeRange" }],
        outputs: [{ name: "timeRange" }],
      },
    },
    input_mapping: { timeRange: "usage.timeRange", other: "other.input" },
    output_mapping: { timeRange: "usage.timeRange", other: "other.output" },
  };
  const original = structuredClone(block);
  const owner = retainedBlockPatch(block, true);
  const view = retainedBlockPatch(block, false);
  assert.deepEqual(block, original);
  assert.deepEqual(owner.runtime_descriptor["x-layout"], { order: 9 });
  assert.equal(owner.runtime_descriptor.codeRef, "existing-code");
  assert.equal(owner.input_mapping.other, "other.input");
  assert.equal(owner.output_mapping.other, "other.output");
  assert.equal(
    owner.output_mapping.reportState,
    view.input_mapping.reportState,
  );
  assert.equal(
    view.runtime_descriptor.ports.outputs.some((p) => p.name === "reportState"),
    false,
  );
  assert.deepEqual(retainedBlockPatch(owner, true), owner);
});
