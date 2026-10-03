// Runs in the workflow Code node. Only normalized ISO timestamps reach SQL.
function main(inputs) {
  const from = inputs.started_from;
  const to = inputs.started_to;
  if (Boolean(from) !== Boolean(to))
    throw new Error("Provide both started_from and started_to");
  const end = to ? new Date(to) : new Date();
  const start = from ? new Date(from) : new Date(end.getTime() - 86400000);
  if (
    !Number.isFinite(start.getTime()) ||
    !Number.isFinite(end.getTime()) ||
    start >= end
  ) {
    throw new Error("Invalid time range");
  }
  return {
    started_from: start.toISOString(),
    started_to: end.toISOString(),
    bucket: end.getTime() - start.getTime() <= 172800000 ? "hour" : "day",
  };
}
if (typeof module !== "undefined") module.exports = { main };
