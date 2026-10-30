"use strict";
const assert = require("node:assert/strict");
const path = require("node:path");
const {
  ids,
  initialDocument,
  nestedDocument,
  assertRun,
} = require("./fixture.cjs");

const {
  until,
  canvasNode,
  fit,
  editorReady,
  number,
  errorPolicy,
  screenshot,
  save,
  selectNode,
  drag,
  submitRun,
} = require("./interactions.cjs");
const { exerciseHistory } = require("./history.cjs");
const { selectRound } = require("./trajectory.cjs");
const semanticGraph = (document) => ({
  nodes: document.graph.nodes.map(
    ({ position, width, height, ...node }) => node,
  ),
  edges: document.graph.edges.map(({ points, ...edge }) => edge),
});

async function runBrowserScene({
  browser,
  api,
  applicationId,
  webBaseUrl,
  outputDir,
  cookies,
  write,
  receipt,
}) {
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    locale: "en-US",
    recordVideo: {
      dir: path.join(outputDir, "videos"),
      size: { width: 1600, height: 1000 },
    },
  });
  await context.addCookies(cookies);
  const page = await context.newPage();
  page.setDefaultTimeout(45000);
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message.slice(0, 500)));
  const route = `${webBaseUrl}/applications/${applicationId}/orchestration`;
  const endpoint = `/api/console/applications/${applicationId}/orchestration`;
  const getState = () => api("GET", endpoint);
  const saveFixture = (document) =>
    api("PUT", `${endpoint}/draft`, {
      document,
      change_kind: "logical",
      summary: "Bounded containers browser fixture preparation",
    });
  let runId;
  try {
    let state = await getState();
    await saveFixture(
      initialDocument(state.draft.document, receipt.application_name),
    );
    await page.goto(route, { waitUntil: "domcontentloaded" });
    const source = canvasNode(page, ids.input);
    await source.waitFor();
    // This is the real plus picker on an ordinary node, not a document or DOM injection.
    await source
      .getByRole("button", {
        name: /在 BC Input 后新增节点|Add node after BC Input/,
      })
      .click();
    const loopOption = page.getByRole("menuitem", { name: /^(Loop|循环)$/ });
    await loopOption.click();
    await page.locator(".agent-flow-container__header").first().click();
    await page
      .getByRole("textbox", { name: /^(节点别名|Node alias)$/ })
      .fill("BC Loop");
    await number(page, /^(最大轮数|Maximum number of rounds)$/, 2);
    await number(
      page,
      /^(单次最大循环（分钟）|Maximum time per round \(minutes\))$/,
      0.5,
    );
    state = await save(
      page,
      getState,
      (document) =>
        document.graph.nodes.some(
          (node) =>
            node.type === "loop" &&
            node.alias === "BC Loop" &&
            node.config.max_rounds === 2 &&
            node.config.round_timeout_ms === 30000,
        ),
      "UI-created loop persisted",
    );
    const loop = state.draft.document.graph.nodes.find(
      (node) => node.type === "loop" && node.alias === "BC Loop",
    );
    const loopId = loop.id;
    assert.equal(
      state.draft.document.graph.nodes.filter(
        (node) =>
          node.containerId === loopId && node.type === "container_start",
      ).length,
      1,
    );
    receipt.ui_created_loop_id = loopId;
    await write("01-ui-created-document.json", state.draft.document);
    await screenshot(page, outputDir, "01-desktop-ui-created-loop.png");

    // API preparation adds the rest of the deterministic graph around the saved UI-created node.
    await page.goto("about:blank");
    await saveFixture(nestedDocument(state.draft.document, loopId));
    await page.goto(route, { waitUntil: "domcontentloaded" });
    await editorReady(page, [
      ids.start,
      loopId,
      ids.iteration,
      ids.innerLoop,
      ids.leaf,
    ]);
    await fit(page);
    await screenshot(page, outputDir, "02-desktop-overview.png");
    await selectNode(page, loopId, "BC Loop", receipt, true);
    await number(page, /^(最大轮数|Maximum number of rounds)$/, 2);
    await number(
      page,
      /^(单次最大循环（分钟）|Maximum time per round \(minutes\))$/,
      0.5,
    );
    await errorPolicy(page, /^(继续下一轮|Continue with the next round)$/);
    await screenshot(page, outputDir, "02a-loop-settings.png");
    await selectNode(page, ids.iteration, "BC Iteration", receipt, true);
    await number(
      page,
      /^(单次最大循环（分钟）|Maximum time per round \(minutes\))$/,
      0.25,
    );
    await errorPolicy(
      page,
      /^(退出当前容器，外层继续|Exit this container and continue outside)$/,
    );
    // Validate actual array variable discovery and Cascader selection from a legitimate Code output.
    await page
      .getByRole("combobox", { name: /^(迭代输入|Iteration input)$/ })
      .click();
    const cascader = page.locator(".ant-cascader-dropdown:visible").last();
    for (const label of ["BC Input", "result", "items"])
      await cascader.getByText(label, { exact: true }).click();
    await screenshot(page, outputDir, "02b-iteration-settings.png");
    state = await save(
      page,
      getState,
      (document) => {
        const outer = document.graph.nodes.find((node) => node.id === loopId);
        const inner = document.graph.nodes.find(
          (node) => node.id === ids.iteration,
        );
        return (
          outer.config.max_rounds === 2 &&
          outer.config.round_timeout_ms === 30000 &&
          outer.config.error_policy === "continue" &&
          inner.config.round_timeout_ms === 15000 &&
          inner.config.error_policy === "break" &&
          JSON.stringify(inner.bindings.items?.value) ===
            JSON.stringify([ids.input, "result", "items"])
        );
      },
      "container controls and array selector saved",
    );
    await selectNode(page, ids.innerLoop, "BC Inner Loop", receipt, true);
    assert.equal(
      await page
        .getByRole("spinbutton", {
          name: /^(最大轮数|Maximum number of rounds)$/,
        })
        .inputValue(),
      "1",
    );
    await screenshot(page, outputDir, "02c-inner-loop-settings.png");
    await selectNode(page, ids.leaf, "BC Item", receipt);
    assert.equal(
      await page
        .getByRole("textbox", { name: /^(节点别名|Node alias)$/ })
        .inputValue(),
      "BC Item",
    );
    assert.equal(
      await canvasNode(page, ids.start).count(),
      1,
      "selection must retain the root canvas",
    );
    await screenshot(page, outputDir, "02-desktop-inner-selection.png");
    receipt.three_level_selection = [
      loopId,
      ids.iteration,
      ids.innerLoop,
      ids.leaf,
    ];
    const before = state.draft.document;
    const initialSemantics = semanticGraph(before);
    const beforeLoop = before.graph.nodes.find((node) => node.id === loopId);
    await selectNode(page, loopId, "BC Loop", receipt, true);
    await drag(
      page,
      canvasNode(page, loopId).locator(".agent-flow-container__header"),
      45,
      35,
      "move-outer-container",
      receipt,
      "header",
    );
    state = await save(
      page,
      getState,
      (document) => {
        const current = document.graph.nodes.find((node) => node.id === loopId);
        return (
          current.position.x !== beforeLoop.position.x ||
          current.position.y !== beforeLoop.position.y
        );
      },
      "container drag saved",
    );
    for (const id of [ids.iteration, ids.innerLoop, ids.leaf])
      assert.deepEqual(
        state.draft.document.graph.nodes.find((node) => node.id === id)
          .position,
        before.graph.nodes.find((node) => node.id === id).position,
        "parent drag must not shift child-relative positions",
      );
    await fit(page);
    const beforeResize = state.draft.document.graph.nodes.find(
      (node) => node.id === loopId,
    );
    await drag(
      page,
      canvasNode(page, loopId).locator(".agent-flow-container__resize"),
      45,
      35,
      "resize-outer-container",
      receipt,
    );
    state = await save(
      page,
      getState,
      (document) => {
        const current = document.graph.nodes.find((node) => node.id === loopId);
        return (
          current.width > beforeResize.width &&
          current.height > beforeResize.height
        );
      },
      "container resize saved",
    );
    assert.deepEqual(
      semanticGraph(state.draft.document),
      initialSemantics,
      "movement, panning and resize preserve execution semantics",
    );
    const saved = await exerciseHistory({
      page,
      getState,
      loopId,
      outputDir,
      write,
      receipt,
    });
    assert.deepEqual(
      semanticGraph(saved),
      initialSemantics,
      "copy deletion and history preserve the original executable graph",
    );
    await write("02-saved-document.json", saved);
    await page.reload({ waitUntil: "domcontentloaded" });
    await editorReady(page, [
      ids.start,
      loopId,
      ids.iteration,
      ids.innerLoop,
      ids.leaf,
    ]);
    await fit(page);
    const reloaded = (await getState()).draft.document;
    assert.deepEqual(
      reloaded.graph,
      saved.graph,
      "reload must preserve parent IDs, relative coordinates, dimensions and settings",
    );
    await selectNode(page, ids.iteration, "BC Iteration", receipt, true);
    await screenshot(page, outputDir, "03-desktop-reloaded.png");

    await page.getByRole("button", { name: /^(测试运行|Test Run)$/ }).click();
    const form = page.getByRole("form", {
      name: /Workflow 测试运行表单|Workflow test run form/,
    });
    await form
      .getByLabel(/输入 payload|Input payload/)
      .fill(JSON.stringify({ items_json: "[1,2,3]" }));
    const response = await submitRun(page, form, endpoint, receipt);
    assert.ok(
      response.ok(),
      `real UI debug run returned HTTP ${response.status()}`,
    );
    let detail = (await response.json()).data;
    runId = detail.flow_run.id;
    receipt.run_id = runId;
    await write("receipt.json", receipt);
    detail = await until(
      () => api("GET", `${endpoint}/runs/${runId}/debug-snapshot`),
      (current) => !["pending", "running"].includes(current.flow_run.status),
      "workflow terminal state",
      90000,
    );
    await write("03-run-evidence.json", {
      flow_run: {
        id: detail.flow_run.id,
        status: detail.flow_run.status,
        output_payload: detail.flow_run.output_payload,
        error_payload: detail.flow_run.error_payload,
      },
      node_runs: detail.node_runs.map((run) => ({
        id: run.id,
        node_id: run.node_id,
        status: run.status,
        output_payload: run.output_payload,
        execution_identity: run.execution_identity,
      })),
    });
    receipt.execution = assertRun(detail, loopId);
    await screenshot(page, outputDir, "04-desktop-run-result.png");
    await page
      .getByRole("button", { name: /查看执行 Trace|View execution trace/i })
      .click();
    await page.waitForURL(
      (url) =>
        url.pathname === `/applications/${applicationId}/logs` &&
        url.searchParams.get("run_id") === runId,
    );
    await page
      .getByRole("combobox", {
        name: /按循环轮次筛选|Filter by container round/,
      })
      .waitFor();
    await screenshot(page, outputDir, "07-desktop-run-trace.png");
    await selectRound({ page, detail, loopId, outputDir, receipt });
    assert.deepEqual(errors, [], "browser must not report uncaught errors");
    receipt.desktop = {
      viewport: { width: 1600, height: 1000 },
      page_errors: errors,
      saved_reload_graph_equal: true,
    };

    const mobile = await browser.newContext({
      viewport: { width: 390, height: 844 },
      isMobile: true,
      deviceScaleFactor: 1,
      locale: "en-US",
      recordVideo: {
        dir: path.join(outputDir, "videos"),
        size: { width: 390, height: 844 },
      },
    });
    try {
      await mobile.addCookies(cookies);
      const mobilePage = await mobile.newPage();
      mobilePage.setDefaultTimeout(45000);
      await mobilePage.goto(route, { waitUntil: "domcontentloaded" });
      await editorReady(mobilePage, [
        ids.start,
        loopId,
        ids.iteration,
        ids.innerLoop,
        ids.leaf,
      ]);
      // Current WorkflowEditorPage has no mobile summary branch. Observe its
      // loaded surface without fitting a desktop graph into a phone for the test.
      const overlay = mobilePage.locator(".agent-flow-editor__overlay-status");
      assert.ok((await overlay.innerText()).includes(receipt.application_name));
      receipt.mobile_video = path.relative(
        outputDir,
        await mobilePage.video().path(),
      );
      await screenshot(mobilePage, outputDir, "06-mobile-editor.png");
      const widths = await mobilePage.evaluate(() => ({
        viewport: document.documentElement.clientWidth,
        scroll: document.documentElement.scrollWidth,
      }));
      receipt.mobile = {
        viewport: { width: 390, height: 844 },
        widths,
        observed_strategy: "desktop_canvas_without_mobile_summary",
        design_compliance: "known_existing_deviation",
        design_requirement:
          "DESIGN.md §10.1: at max-width 768px show a summary and desktop-operation guidance",
        source_evidence: [
          "web/app/src/features/workflow/pages/WorkflowEditorPage.tsx",
          "web/app/src/features/flow-editor/components/FlowEditorKernel.tsx",
        ],
        horizontal_overflow: widths.scroll > widths.viewport + 1,
        rendered_node_ids: [
          ids.start,
          loopId,
          ids.iteration,
          ids.innerLoop,
          ids.leaf,
        ],
        scope:
          "loaded editor observation only; mobile summary, guidance and editing are not accepted",
      };
    } finally {
      await mobile.close();
    }
  } catch (error) {
    await screenshot(page, outputDir, "failure-desktop.png").catch(
      () => undefined,
    );
    throw error;
  } finally {
    try {
      await context.close();
    } finally {
      receipt.desktop_video = path.relative(
        outputDir,
        await page.video().path(),
      );
    }
  }
}
module.exports = { runBrowserScene };
