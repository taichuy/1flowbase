'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const INVENTORY_PATH = path.join(__dirname, 'openai-inventory.json');
const EXPECTED_COUNTS = Object.freeze({
  tools: 16,
  tool_choices: 9,
  input_items: 6,
  items: 28,
  output_items: 28,
  stream_events: 53,
});

function loadPinnedInventory(filePath = INVENTORY_PATH) {
  const bytes = fs.readFileSync(filePath);
  const inventory = JSON.parse(bytes);
  if (inventory.source.revision !== '5c044be3bf3a42854e99e34616564eeb2124a317') {
    throw new Error('OpenAI inventory revision is not pinned');
  }
  for (const [group, count] of Object.entries(EXPECTED_COUNTS)) {
    const names = inventory.groups[group];
    if (!Array.isArray(names) || names.length !== count || new Set(names).size !== count) {
      throw new Error(`OpenAI inventory ${group} must contain ${count} unique names`);
    }
  }
  if (!inventory.groups.items.includes(inventory.synthetic_unknown.name)) {
    throw new Error('OpenAI inventory omitted synthetic unknown drift item');
  }
  return {
    ...inventory,
    sha256: crypto.createHash('sha256').update(bytes).digest('hex'),
    counts: Object.fromEntries(Object.keys(EXPECTED_COUNTS).map((group) => [group, inventory.groups[group].length])),
  };
}

module.exports = { EXPECTED_COUNTS, INVENTORY_PATH, loadPinnedInventory };
