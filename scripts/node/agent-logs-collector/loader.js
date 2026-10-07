'use strict';
const path = require('node:path');
function loadAdapter(selection = 'codex') {
  const adapter = selection === 'codex' ? require('./adapters/codex') : require(path.resolve(selection));
  if (!adapter || typeof adapter.sourceClient !== 'string' || typeof adapter.createContext !== 'function' || typeof adapter.convert !== 'function') {
    throw new Error('Adapter must export sourceClient, createContext(firstLine), convert(line, context, position)');
  }
  return adapter;
}
module.exports = { loadAdapter };
