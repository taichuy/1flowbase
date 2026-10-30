"use strict";
const assert = require("node:assert/strict");
const path = require("node:path");

async function until(read, predicate, label, timeoutMs = 45000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await read();
    if (predicate(value)) return value;
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Timed out: ${label}`);
}
const canvasNode = (page, id) =>
  page.locator(`.react-flow__node[data-id="${id}"]`);
async function settled(page) {
  await page.locator(".react-flow__viewport").evaluate(
    (element) =>
      new Promise((resolve, reject) => {
        const started = performance.now();
        let previous = "",
          stable = 0;
        function frame() {
          if (performance.now() - started > 5000) {
            reject(new Error("Canvas animation did not settle"));
            return;
          }
          const current = element.getAttribute("style");
          stable = current === previous ? stable + 1 : 0;
          previous = current;
          if (stable >= 12) resolve();
          else requestAnimationFrame(frame);
        }
        requestAnimationFrame(frame);
      }),
  );
}
async function fit(page) {
  await page
    .getByRole("button", { name: /^(适应画布|Adapt to canvas)$/ })
    .click();
  await settled(page);
}
async function editorReady(page, nodeIds) {
  await page.locator(".agent-flow-canvas .react-flow__viewport").waitFor();
  for (const id of nodeIds) {
    const node = canvasNode(page, id);
    await node.waitFor({ state: "visible" });
    await until(
      () => node.boundingBox(),
      (box) => box && box.width > 5 && box.height > 5,
      `measured canvas node ${id}`,
    );
  }
  await settled(page);
}
async function number(page, name, value) {
  const field = page.getByRole("spinbutton", { name });
  await field.fill(String(value));
  await field.press("Tab");
}
async function errorPolicy(page, choice) {
  await page
    .getByRole("combobox", { name: /^(本轮失败时|When a round fails)$/ })
    .click();
  await page
    .locator(".ant-select-dropdown:visible")
    .getByTitle(choice)
    .last()
    .click();
}
async function screenshot(page, outputDir, name) {
  await page.screenshot({
    path: path.join(outputDir, name),
    animations: "disabled",
    fullPage: true,
  });
}
async function save(page, getState, predicate, label) {
  await page.getByRole("button", { name: /^(保存|save)$/ }).click();
  return until(getState, (state) => predicate(state.draft.document), label);
}
async function targetPoint(locator, kind = "center") {
  return locator.evaluate((element, kind) => {
    const box = element.getBoundingClientRect();
    const x =
      box.left +
      (kind === "header" ? Math.min(60, box.width / 2) : box.width / 2);
    const y = box.top + box.height / 2;
    const top = document.elementFromPoint(x, y);
    return {
      x,
      y,
      width: box.width,
      height: box.height,
      hit: !!top && (top === element || element.contains(top)),
      hit_class: top?.getAttribute("class") ?? null,
    };
  }, kind);
}
async function panToExpose(page, point) {
  const geometry = await page
    .locator(".agent-flow-canvas")
    .evaluate((canvas) => {
      const rect = canvas.getBoundingClientRect();
      const dock = document
        .querySelector(".agent-flow-editor__detail-dock")
        ?.getBoundingClientRect();
      const right =
        dock && dock.width > 0 ? Math.min(rect.right, dock.left) : rect.right;
      const safe = {
        left: rect.left + 30,
        right: right - 90,
        top: rect.top + 90,
        bottom: rect.bottom - 90,
      };
      const candidates = [];
      for (const y of [safe.top + 30, safe.bottom - 30])
        for (let x = safe.left + 20; x < safe.right; x += 60)
          candidates.push({ x, y });
      const start = candidates.find(({ x, y }) =>
        document.elementFromPoint(x, y)?.classList.contains("react-flow__pane"),
      );
      return { safe, start };
    });
  assert.ok(
    geometry.start,
    "A real exposed canvas pane is required for middle-button panning",
  );
  const desired = {
    x: (geometry.safe.left + geometry.safe.right) / 2,
    y: (geometry.safe.top + geometry.safe.bottom) / 2,
  };
  const dx = Math.max(-250, Math.min(250, desired.x - point.x));
  const dy = Math.max(-180, Math.min(180, desired.y - point.y));
  assert.ok(
    Math.abs(dx) + Math.abs(dy) > 2,
    `Target is occluded at safe center by ${point.hit_class}`,
  );
  await page.mouse.move(geometry.start.x, geometry.start.y);
  await page.mouse.down({ button: "middle" });
  try {
    await page.mouse.move(geometry.start.x + dx, geometry.start.y + dy, {
      steps: 12,
    });
  } finally {
    await page.mouse.up({ button: "middle" });
  }
  await settled(page);
}
async function ensureHit(page, locator, label, receipt, kind = "center") {
  await locator.waitFor({ state: "visible" });
  const attempts = [];
  for (let attempt = 0; attempt < 8; attempt++) {
    const point = await targetPoint(locator, kind);
    attempts.push(point);
    if (point.hit && point.width > 0 && point.height > 0) {
      receipt.pointer_targets ??= [];
      receipt.pointer_targets.push({ label, attempts });
      return point;
    }
    await panToExpose(page, point);
  }
  throw new Error(
    `${label}: pointer never reached the actual control (${JSON.stringify(attempts)})`,
  );
}
async function selectNode(page, nodeId, alias, receipt, container = false) {
  const control = canvasNode(page, nodeId).locator(
    container ? ".agent-flow-container__header" : ".agent-flow-node-card",
  );
  const point = await ensureHit(
    page,
    control,
    `select:${nodeId}`,
    receipt,
    container ? "header" : "center",
  );
  await page.mouse.click(point.x, point.y);
  const field = page.getByRole("textbox", { name: /^(节点别名|Node alias)$/ });
  await until(
    () => field.inputValue(),
    (value) => value === alias,
    `inspector selection ${alias}`,
  );
}
async function drag(page, locator, dx, dy, label, receipt, kind = "center") {
  const point = await ensureHit(page, locator, label, receipt, kind);
  await page.mouse.move(point.x, point.y);
  await page.mouse.down();
  try {
    await page.mouse.move(point.x + dx, point.y + dy, { steps: 16 });
  } finally {
    await page.mouse.up();
  }
  await settled(page);
}
// Use the real form submit control, independent of AntD's Chinese character spacing.
// The listener always has a rejection handler and is removed even when click fails.
async function submitRun(page, form, endpoint, receipt) {
  let listener, timer, stopWait;
  const responsePromise = new Promise((resolve, reject) => {
    stopWait = reject;
    listener = (response) => {
      if (
        response.request().method() === "POST" &&
        new URL(response.url()).pathname === `${endpoint}/debug-runs`
      )
        resolve(response);
    };
    page.on("response", listener);
    timer = setTimeout(
      () =>
        reject(
          new Error("Timed out waiting for the formal debug-run response"),
        ),
      90000,
    );
  });
  void responsePromise.catch(() => undefined);
  try {
    const button = form.locator('button[type="submit"]');
    assert.equal(await button.count(), 1);
    receipt.run_submit_text = (await button.innerText()).replace(/\s+/g, "");
    assert.match(receipt.run_submit_text, /^(运行|Run)$/);
    await button.click();
    return await responsePromise;
  } finally {
    clearTimeout(timer);
    page.off("response", listener);
    stopWait(new Error("Run response listener disposed"));
  }
}
module.exports = {
  until,
  canvasNode,
  settled,
  fit,
  editorReady,
  number,
  errorPolicy,
  screenshot,
  save,
  ensureHit,
  selectNode,
  drag,
  submitRun,
};
