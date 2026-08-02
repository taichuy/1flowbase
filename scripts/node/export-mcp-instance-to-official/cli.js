const path = require('node:path');
const { exportMcpInstanceToOfficial } = require('./core.js');

const USAGE = 'usage: node scripts/node/export-mcp-instance-to-official.js --instance-id <id> --target <absolute mcp/@org/bundle> [--api-base-url <url>]';

function readArguments(argv) {
  if (argv.length === 1 && (argv[0] === '-h' || argv[0] === '--help')) return { help: true };
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
    throw new Error(USAGE);
  }
  if (!path.isAbsolute(target)) throw new Error('--target must be an absolute path');
  return {
    instanceId,
    target,
    apiBaseUrl: values.get('--api-base-url') || 'http://127.0.0.1:7800',
  };
}

async function main(argv = process.argv.slice(2), dependencies = {}) {
  const options = readArguments(argv);
  if (options.help) {
    process.stdout.write(`${USAGE}\n`);
    return;
  }
  const result = await exportMcpInstanceToOfficial(options, dependencies);
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}

module.exports = { USAGE, main, readArguments };
