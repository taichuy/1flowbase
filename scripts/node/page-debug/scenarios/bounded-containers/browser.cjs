'use strict';
const assert = require('node:assert/strict');
const path = require('node:path');
const { ids, initialDocument, nestedDocument, assertRun } = require('./fixture.cjs');

async function until(read, predicate, label, timeoutMs = 45000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await read();
    if (predicate(value)) return value;
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Timed out: ${label}`);
}
const canvasNode = (page, id) => page.locator(`.react-flow__node[data-id="${id}"]`);
async function fit(page) {
  await page.getByRole('button', { name: /^(适应画布|Adapt to canvas)$/ }).click();
  // Wait on observable animation stability, not on arbitrary wall time.
  await page.locator('.react-flow__viewport').evaluate((element) => new Promise((resolve, reject) => {
    const started = performance.now();
    let previous = '', stable = 0;
    function frame() {
      if (performance.now() - started > 5000) { reject(new Error('Canvas fit animation did not settle')); return; }
      const current = element.getAttribute('style');
      stable = current === previous ? stable + 1 : 0;
      previous = current;
      if (stable >= 20) resolve(); else requestAnimationFrame(frame);
    }
    requestAnimationFrame(frame);
  }));
}
async function number(page, name, value) {
  const field = page.getByRole('spinbutton', { name });
  await field.fill(String(value));
  await field.press('Tab');
}
async function errorPolicy(page, choice) {
  await page.getByRole('combobox', { name: /^(本轮失败时|When a round fails)$/ }).click();
  await page.locator('.ant-select-dropdown:visible').getByTitle(choice).last().click();
}
async function screenshot(page, outputDir, name) {
  await page.screenshot({ path: path.join(outputDir, name), animations: 'disabled', fullPage: true });
}
async function save(page, getState, predicate, label) {
  await page.getByRole('button', { name: /^(保存|save)$/ }).click();
  return until(getState, (state) => predicate(state.draft.document), label);
}
async function drag(page, locator, dx, dy) {
  const box = await locator.boundingBox();
  assert.ok(box && box.width > 0 && box.height > 0, 'drag handle must be visible');
  const x = box.x + Math.min(60, box.width / 2), y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx, y + dy, { steps: 16 });
  await page.mouse.up();
}

async function runBrowserScene({ browser, api, applicationId, webBaseUrl, outputDir, cookies, write, receipt }) {
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 }, locale: 'en-US' });
  await context.addCookies(cookies);
  const page = await context.newPage();
  page.setDefaultTimeout(45000);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message.slice(0, 500)));
  const route = `${webBaseUrl}/applications/${applicationId}/orchestration`;
  const endpoint = `/api/console/applications/${applicationId}/orchestration`;
  const getState = () => api('GET', endpoint);
  const saveFixture = (document) => api('PUT', `${endpoint}/draft`, { document, change_kind: 'logical', summary: 'Bounded containers browser fixture preparation' });
  let runId;
  try {
    let state = await getState();
    await saveFixture(initialDocument(state.draft.document, receipt.application_name));
    await page.goto(route, { waitUntil: 'domcontentloaded' });
    const source = canvasNode(page, ids.input);
    await source.waitFor();
    // This is the real plus picker on an ordinary node, not a document or DOM injection.
    await source.getByRole('button', { name: /在 BC Input 后新增节点|Add node after BC Input/ }).click();
    const loopOption = page.getByRole('menuitem', { name: /^(Loop|循环)$/ });
    await loopOption.click();
    await page.locator('.agent-flow-container__header').first().click();
    await page.getByRole('textbox', { name: /^(节点别名|Node alias)$/ }).fill('BC Loop');
    await number(page, /^(最大轮数|Maximum number of rounds)$/, 2);
    await number(page, /^(每轮期限（毫秒）|Round deadline \(milliseconds\))$/, 30000);
    state = await save(page, getState, (document) => document.graph.nodes.some((node) => node.type === 'loop' && node.alias === 'BC Loop' && node.config.max_rounds === 2 && node.config.round_timeout_ms === 30000), 'UI-created loop persisted');
    const loop = state.draft.document.graph.nodes.find((node) => node.type === 'loop' && node.alias === 'BC Loop');
    const loopId = loop.id;
    assert.equal(state.draft.document.graph.nodes.filter((node) => node.containerId === loopId && node.type === 'container_start').length, 1);
    receipt.ui_created_loop_id = loopId;
    await write('01-ui-created-document.json', state.draft.document);
    await screenshot(page, outputDir, '01-desktop-ui-created-loop.png');

    // API preparation adds the rest of the deterministic graph around the saved UI-created node.
    await page.goto('about:blank');
    await saveFixture(nestedDocument(state.draft.document, loopId));
    await page.goto(route, { waitUntil: 'domcontentloaded' });
    await canvasNode(page, ids.leaf).waitFor();
    await fit(page);
    await canvasNode(page, loopId).locator('.agent-flow-container__header').click();
    await number(page, /^(最大轮数|Maximum number of rounds)$/, 2);
    await number(page, /^(每轮期限（毫秒）|Round deadline \(milliseconds\))$/, 30000);
    await errorPolicy(page, /^(继续下一轮|Continue with the next round)$/);
    await canvasNode(page, ids.iteration).locator('.agent-flow-container__header').click();
    await number(page, /^(每轮期限（毫秒）|Round deadline \(milliseconds\))$/, 15000);
    await errorPolicy(page, /^(退出当前容器，外层继续|Exit this container and continue outside)$/);
    // Validate actual array variable discovery and Cascader selection from a legitimate Code output.
    await page.getByRole('combobox', { name: /^(迭代输入|Iteration input)$/ }).click();
    const cascader = page.locator('.ant-cascader-dropdown:visible').last();
    for (const label of ['BC Input', 'result', 'items']) await cascader.getByText(label, { exact: true }).click();
    state = await save(page, getState, (document) => {
      const outer = document.graph.nodes.find((node) => node.id === loopId);
      const inner = document.graph.nodes.find((node) => node.id === ids.iteration);
      return outer.config.max_rounds === 2 && outer.config.round_timeout_ms === 30000 && outer.config.error_policy === 'continue' && inner.config.round_timeout_ms === 15000 && inner.config.error_policy === 'break' && JSON.stringify(inner.bindings.items?.value) === JSON.stringify([ids.input, 'result', 'items']);
    }, 'container controls and array selector saved');
    await canvasNode(page, ids.leaf).locator('.agent-flow-node-card').click();
    assert.equal(await page.getByRole('textbox', { name: /^(节点别名|Node alias)$/ }).inputValue(), 'BC Item');
    assert.equal(await canvasNode(page, ids.start).count(), 1, 'selection must retain the root canvas');
    await screenshot(page, outputDir, '02-desktop-inner-selection.png');
    const before = state.draft.document;
    const beforeLoop = before.graph.nodes.find((node) => node.id === loopId);
    await canvasNode(page, loopId).locator('.agent-flow-container__header').click();
    await drag(page, canvasNode(page, loopId).locator('.agent-flow-container__header'), 45, 35);
    state = await save(page, getState, (document) => {
      const current = document.graph.nodes.find((node) => node.id === loopId);
      return current.position.x !== beforeLoop.position.x || current.position.y !== beforeLoop.position.y;
    }, 'container drag saved');
    for (const id of [ids.iteration, ids.leaf]) assert.deepEqual(state.draft.document.graph.nodes.find((node) => node.id === id).position, before.graph.nodes.find((node) => node.id === id).position, 'parent drag must not shift child-relative positions');
    await fit(page);
    const beforeResize = state.draft.document.graph.nodes.find((node) => node.id === loopId);
    await drag(page, canvasNode(page, loopId).locator('.agent-flow-container__resize'), 45, 35);
    state = await save(page, getState, (document) => {
      const current = document.graph.nodes.find((node) => node.id === loopId);
      return current.width > beforeResize.width && current.height > beforeResize.height;
    }, 'container resize saved');
    const saved = state.draft.document;
    await write('02-saved-document.json', saved);
    await page.reload({ waitUntil: 'domcontentloaded' });
    await canvasNode(page, ids.leaf).waitFor();
    await fit(page);
    const reloaded = (await getState()).draft.document;
    assert.deepEqual(reloaded.graph, saved.graph, 'reload must preserve parent IDs, relative coordinates, dimensions and settings');
    await canvasNode(page, ids.iteration).locator('.agent-flow-container__header').click();
    await screenshot(page, outputDir, '03-desktop-reloaded.png');

    await page.getByRole('button', { name: /^(测试运行|Test Run)$/ }).click();
    const form = page.getByRole('form', { name: /Workflow 测试运行表单|Workflow test run form/ });
    await form.getByLabel(/输入 payload|Input payload/).fill(JSON.stringify({ items_json: '[1,2,3]' }));
    const responsePromise = page.waitForResponse((response) => response.request().method() === 'POST' && new URL(response.url()).pathname === `${endpoint}/debug-runs`, { timeout: 90000 });
    await form.getByRole('button', { name: /^(运行|Run)$/ }).click();
    const response = await responsePromise;
    assert.ok(response.ok(), `real UI debug run returned HTTP ${response.status()}`);
    let detail = (await response.json()).data;
    runId = detail.flow_run.id;
    receipt.run_id = runId;
    await write('receipt.json', receipt);
    detail = await until(() => api('GET', `${endpoint}/runs/${runId}/debug-snapshot`), (current) => !['pending', 'running'].includes(current.flow_run.status), 'workflow terminal state', 90000);
    receipt.execution = assertRun(detail, loopId);
    await write('03-run-evidence.json', { flow_run: { id: detail.flow_run.id, status: detail.flow_run.status, output_payload: detail.flow_run.output_payload }, node_runs: detail.node_runs.map((run) => ({ id: run.id, node_id: run.node_id, status: run.status, output_payload: run.output_payload, execution_identity: run.execution_identity })) });
    await screenshot(page, outputDir, '04-desktop-run-result.png');
    await page.getByRole('button', { name: /查看执行 Trace|View execution trace/i }).click();
    await page.waitForURL((url) => url.pathname === `/applications/${applicationId}/logs` && url.searchParams.get('run_id') === runId);
    await page.getByRole('combobox', { name: /按循环轮次筛选|Filter by container round/ }).waitFor();
    await screenshot(page, outputDir, '05-desktop-run-trace.png');
    assert.deepEqual(errors, [], 'browser must not report uncaught errors');
    receipt.desktop = { viewport: { width: 1600, height: 1000 }, page_errors: errors, saved_reload_graph_equal: true };

    const mobile = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, deviceScaleFactor: 1, locale: 'en-US' });
    try {
      await mobile.addCookies(cookies);
      const mobilePage = await mobile.newPage();
      mobilePage.setDefaultTimeout(45000);
      await mobilePage.goto(route, { waitUntil: 'domcontentloaded' });
      await mobilePage.locator('.agent-flow-editor__body').waitFor();
      await screenshot(mobilePage, outputDir, '06-mobile-editor.png');
      const widths = await mobilePage.evaluate(() => ({ viewport: document.documentElement.clientWidth, scroll: document.documentElement.scrollWidth }));
      receipt.mobile = { viewport: { width: 390, height: 844 }, widths, scope: 'layout screenshot; no mobile editing claim' };
      assert.ok(widths.scroll <= widths.viewport + 1, 'mobile page overflows horizontally');
    } finally { await mobile.close(); }
  } catch (error) {
    await screenshot(page, outputDir, 'failure-desktop.png').catch(() => undefined);
    throw error;
  } finally { await context.close(); }
}
module.exports = { runBrowserScene };
