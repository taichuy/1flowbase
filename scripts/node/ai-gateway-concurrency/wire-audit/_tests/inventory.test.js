'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { EXPECTED_COUNTS, loadPinnedInventory } = require('../inventory');

// Root AC-015 / D3 WireAudit: counts and exact names are pinned data, not labels.
test('pinned OpenAI inventory has the complete finite groups and synthetic drift', () => {
  const inventory = loadPinnedInventory();
  assert.deepEqual(inventory.counts, EXPECTED_COUNTS);
  assert.equal(inventory.pinned_known_counts.items, 27);
  assert.equal(inventory.counts.items, inventory.pinned_known_counts.items + 1);
  assert.deepEqual(inventory.groups.tools.slice(-4), [
    'NamespaceToolParam', 'ToolSearchToolParam', 'WebSearchPreviewTool', 'ApplyPatchToolParam',
  ]);
  assert.deepEqual(inventory.groups.tool_choices.slice(-3), [
    'SpecificProgrammaticToolCallingParam', 'SpecificApplyPatchParam', 'SpecificFunctionShellParam',
  ]);
  assert.ok(inventory.groups.stream_events.includes('ResponseMCPCallArgumentsDeltaEvent'));
  assert.ok(inventory.groups.items.includes('SyntheticUnknownItemDrift'));
  assert.match(inventory.sha256, /^[a-f0-9]{64}$/u);
});
