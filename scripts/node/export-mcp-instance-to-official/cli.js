const path = require('node:path');
const { exportMcpInstanceToOfficial } = require('./core.js');

function readArguments(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith('--') || !value) throw new Error(`invalid argument near ${name || '<end>'}`);
    values.set(name, value);
  }
  const instanceId = values.get('--instance-id');
  const target = values.get('--target');
  if (!instanceId || !target) {
    throw new Error('usage: node scripts/node/export-mcp-instance-to-official.js --instance-id <id> --target <absolute mcp/@org/bundle> [--api-base-url <url>]');
  }
  if (!path.isAbsolute(target)) throw new Error('--target must be an absolute path');
  return {
    instanceId,
    target,
    apiBaseUrl: values.get('--api-base-url') || 'http://127.0.0.1:3000',
  };
}

async function main(argv = process.argv.slice(2), dependencies = {}) {
  const result = await exportMcpInstanceToOfficial(readArguments(argv), dependencies);
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}

module.exports = { main, readArguments };
