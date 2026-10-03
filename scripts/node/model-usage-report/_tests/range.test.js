const { test } = require("node:test");
const assert = require("node:assert/strict");
const { main } = require("../range");
test("default is an exact rolling 24 hours", () => {
  const r = main({});
  assert.equal(Date.parse(r.started_to) - Date.parse(r.started_from), 86400000);
  assert.equal(r.bucket, "hour");
});
test("custom normalizes timezone before SQL and chooses daily bucket", () => {
  assert.deepEqual(
    main({
      started_from: "2026-09-01T08:00:00+08:00",
      started_to: "2026-10-01T08:00:00+08:00",
    }),
    {
      started_from: "2026-09-01T00:00:00.000Z",
      started_to: "2026-10-01T00:00:00.000Z",
      bucket: "day",
    },
  );
});
test("rejects partial, reversed, invalid and injected ranges", () => {
  for (const [from, to] of [
    ["2026-01-01", null],
    ["2026-02-01", "2026-01-01"],
    ["2026-01-01", "2026-01-01"],
    ["';drop table x;--", "2026-01-01"],
  ])
    assert.throws(() => main({ started_from: from, started_to: to }));
});
