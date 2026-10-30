"use strict";
const assert = require("node:assert/strict");
const { ids } = require("./fixture.cjs");
const {
  canvasNode,
  save,
  selectNode,
  until,
  screenshot,
} = require("./interactions.cjs");

function descendants(document, rootId) {
  const selected = new Set([rootId]);
  for (const id of selected)
    for (const node of document.graph.nodes)
      if (node.containerId === id) selected.add(node.id);
  return document.graph.nodes.filter((node) => selected.has(node.id));
}
function assertCopy(before, copied, rootId) {
  const originals = descendants(before, rootId);
  const existing = new Set(before.graph.nodes.map((node) => node.id));
  const copies = copied.graph.nodes.filter((node) => !existing.has(node.id));
  assert.equal(
    copies.length,
    originals.length,
    "copy preserves the whole three-level subtree",
  );
  const root = originals.find((node) => node.id === rootId);
  const copyRoot = copies.find(
    (node) => node.containerId === root.containerId && node.type === root.type,
  );
  assert.ok(copyRoot);
  const mapping = new Map([[root.id, copyRoot.id]]);
  for (const [originalId, copiedId] of mapping) {
    for (const child of originals.filter(
      (node) => node.containerId === originalId,
    )) {
      const matches = copies.filter(
        (node) => node.containerId === copiedId && node.type === child.type,
      );
      assert.equal(
        matches.length,
        1,
        `fixture has one ${child.type} child in each level`,
      );
      mapping.set(child.id, matches[0].id);
    }
  }
  assert.equal(mapping.size, originals.length);
  assert.notDeepEqual(copyRoot.position, root.position, "copy moves the root");
  for (const original of originals) {
    const copy = copies.find((node) => node.id === mapping.get(original.id));
    if (original.id !== rootId)
      assert.deepEqual(
        copy.position,
        original.position,
        "child-relative coordinates receive no duplicate offset",
      );
    assert.deepEqual(copy.config, original.config);
    assert.equal(copy.width, original.width);
    assert.equal(copy.height, original.height);
  }
  const byId = new Map(copies.map((node) => [node.id, node]));
  assert.deepEqual(copyRoot.bindings.output.value, [
    mapping.get(ids.iteration),
    "result",
  ]);
  assert.deepEqual(byId.get(mapping.get(ids.iteration)).bindings.output.value, [
    mapping.get(ids.innerLoop),
    "result",
  ]);
  assert.deepEqual(byId.get(mapping.get(ids.innerLoop)).bindings.output.value, [
    mapping.get(ids.leaf),
    "text",
  ]);
  assert.deepEqual(
    byId.get(mapping.get(ids.iteration)).bindings.items.value,
    [ids.input, "result", "items"],
    "legal external binding remains external",
  );
  assert.equal(
    byId.get(mapping.get(ids.leaf)).bindings.template.value,
    `{{${copyRoot.id}.index}}:{{${mapping.get(ids.iteration)}.item}}`,
  );
  const sourceIds = new Set(originals.map((node) => node.id));
  const originalEdges = before.graph.edges.filter(
    (edge) => sourceIds.has(edge.source) && sourceIds.has(edge.target),
  );
  for (const original of originalEdges)
    assert.ok(
      copied.graph.edges.some(
        (edge) =>
          edge.source === mapping.get(original.source) &&
          edge.target === mapping.get(original.target) &&
          edge.containerId === mapping.get(original.containerId),
      ),
    );
  return {
    root_id: copyRoot.id,
    node_ids: copies.map((node) => node.id),
    id_mapping: Object.fromEntries(mapping),
  };
}
async function exerciseHistory({
  page,
  getState,
  loopId,
  outputDir,
  write,
  receipt,
}) {
  await selectNode(page, loopId, "BC Loop", receipt, true);
  const before = (await save(page, getState, () => true, "history baseline"))
    .draft.document;
  const initialCount = before.graph.nodes.length;
  // Canvas header remains the selection owner. The Save button is not an editable input.
  await page.keyboard.press("Control+c");
  await page.keyboard.press("Control+v");
  await until(
    () => page.locator(".react-flow__node").count(),
    (count) => count > initialCount,
    "copied subtree rendered",
  );
  const copied = (
    await save(
      page,
      getState,
      (document) => document.graph.nodes.length > initialCount,
      "copied subtree persisted",
    )
  ).draft.document;
  const proof = assertCopy(before, copied, loopId);
  await write("04-copied-document.json", copied);
  await screenshot(page, outputDir, "04-three-level-copy.png");

  // Avoid fit/pan between history actions: viewport changes are legitimate editor history.
  await page.keyboard.press("Delete");
  await canvasNode(page, proof.root_id).waitFor({ state: "detached" });
  const deleted = (
    await save(
      page,
      getState,
      (document) =>
        !document.graph.nodes.some((node) => node.id === proof.root_id),
      "copied subtree deleted",
    )
  ).draft.document;
  assert.deepEqual(
    deleted.graph,
    before.graph,
    "deleting the copy preserves the original graph and references",
  );
  await page.keyboard.press("Control+z");
  await canvasNode(page, proof.root_id).waitFor();
  const undone = (
    await save(
      page,
      getState,
      (document) =>
        document.graph.nodes.some((node) => node.id === proof.root_id),
      "deletion undone",
    )
  ).draft.document;
  assert.deepEqual(
    undone.graph,
    copied.graph,
    "one undo restores all descendants, internal edges and references",
  );
  await page.keyboard.press("Control+Shift+z");
  await canvasNode(page, proof.root_id).waitFor({ state: "detached" });
  const redone = (
    await save(
      page,
      getState,
      (document) =>
        !document.graph.nodes.some((node) => node.id === proof.root_id),
      "deletion redone",
    )
  ).draft.document;
  assert.deepEqual(
    redone.graph,
    before.graph,
    "one redo removes precisely the copied subtree",
  );
  await write("05-history-final-document.json", redone);
  receipt.history = {
    ...proof,
    delete_restored_original: true,
    undo_restored_copy: true,
    redo_restored_original: true,
  };
  return redone;
}
module.exports = { exerciseHistory };
