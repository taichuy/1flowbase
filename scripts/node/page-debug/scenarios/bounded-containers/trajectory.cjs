"use strict";
const assert = require("node:assert/strict");
const { ids } = require("./fixture.cjs");
const { until, screenshot } = require("./interactions.cjs");

async function selectRound({ page, detail, loopId, outputDir, receipt }) {
  const leaf = detail.node_runs.find(
    (run) =>
      run.node_id === ids.leaf &&
      run.execution_identity?.round_path
        .map((part) => part.round_index)
        .join("/") === "1/1/0",
  );
  assert.ok(leaf, "canonical third-level 1/1/0 execution exists");
  const filter = page.getByRole("combobox", {
    name: /按循环轮次筛选|Filter by container round/,
  });
  await filter.click();
  const popup = page.locator(".ant-select-dropdown:visible").last();
  const titles = [
    /^BC Loop · (第 2 轮|Round 2)$/,
    /^BC Iteration · (第 2 轮|Round 2)$/,
    /^BC Inner Loop · (第 1 轮|Round 1)$/,
  ];
  for (const [index, title] of titles.entries()) {
    const content = popup.getByTitle(title, { exact: true });
    await content.waitFor();
    if (index === titles.length - 1) await content.click();
    else {
      const branch = content.locator("..");
      const closed = branch.locator(".ant-select-tree-switcher_close");
      if (await closed.count()) await closed.click();
    }
  }
  await popup.waitFor({ state: "hidden" });
  const rows = page
    .locator(".provider-trajectory__row")
    .filter({ hasText: "BC Item" });
  await until(
    () => rows.count(),
    (count) => count > 0,
    "filtered real trajectory leaf rows",
  );
  const labels = await rows.evaluateAll((elements) =>
    elements.map((element) => ({
      label: element.getAttribute("aria-label"),
      text: element.textContent,
    })),
  );
  for (const row of labels) {
    assert.ok(
      row.label.endsWith(leaf.id),
      "round filter must identify the selected node-run receipt",
    );
    assert.match(row.text, /BC Loop · (第 2 轮|Round 2)/);
    assert.match(row.text, /BC Iteration · (第 2 轮|Round 2)/);
    assert.match(row.text, /BC Inner Loop · (第 1 轮|Round 1)/);
  }
  await rows.last().click();
  assert.equal(await rows.last().getAttribute("aria-pressed"), "true");
  await screenshot(page, outputDir, "08-desktop-selected-round.png");
  receipt.trajectory_selection = {
    loop_id: loopId,
    path: leaf.execution_identity.round_path,
    node_run_id: leaf.id,
    execution_id: leaf.execution_identity.execution_id,
    visible_leaf_rows: labels,
  };
}
module.exports = { selectRound };
