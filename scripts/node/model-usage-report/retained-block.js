const reportStateSchema = {
  type: "object",
  required: ["report", "busy", "error"],
  properties: {
    report: {},
    busy: { type: "boolean" },
    error: { type: "boolean" },
  },
};

function retainedBlockPatch(block, owner) {
  const descriptor = structuredClone(block.runtime_descriptor);
  descriptor.ports = {
    ...descriptor.ports,
    inputs: [
      ...(descriptor.ports?.inputs || []).filter(
        (p) => p.name !== "reportState",
      ),
      { name: "reportState", schema: reportStateSchema },
    ],
    outputs: [
      ...(descriptor.ports?.outputs || []).filter(
        (p) => p.name !== "reportState",
      ),
      ...(owner ? [{ name: "reportState", schema: reportStateSchema }] : []),
    ],
  };
  return {
    runtime_descriptor: descriptor,
    input_mapping: { ...block.input_mapping, reportState: "usage.reportState" },
    output_mapping: {
      ...block.output_mapping,
      ...(owner ? { reportState: "usage.reportState" } : {}),
    },
  };
}

module.exports = { retainedBlockPatch };
