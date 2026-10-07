#!/usr/bin/env node
'use strict';
const path = require('node:path');
const { setTimeout: delay } = require('node:timers/promises');
const { collect } = require('./agent-logs-collector/collector');
const HELP = `Agent Logs source CLI (Node.js 20+; no npm release package is assumed)
Usage: node scripts/node/agent-logs-collector.js import|watch --endpoint URL --source PATH [options]
  --endpoint URL  Exact ingest URL, e.g. https://host/api/logs/v1/events
  --source PATH   Explicit Codex .jsonl file or recursively scanned directory
  --state PATH    Durable checkpoint (default: .agent-logs-collector-state.json in cwd)
  --source-id ID  Stable installation identity; generated and persisted if omitted
  --adapter PATH Installed trusted CommonJS adapter script (default: codex)
  --batch-size N Transport batch size (default: 100; no total file/event cap)
  --interval-ms N Watch polling interval (default: 2000)
Credential: FLOWBASE_AGENT_LOGS_API_KEY environment variable; never a CLI argument.
Examples:
  node scripts/node/agent-logs-collector.js import --endpoint https://host/api/logs/v1/events --source /chosen/codex --state /private/collector.json
  node scripts/node/agent-logs-collector.js watch --endpoint https://host/api/logs/v1/events --source /chosen/codex/sessions --state /private/collector.json
Select the Codex root to include both sessions and archived_sessions.
`;
function parse(argv) {
  if (argv.includes('--help') || argv.includes('-h')) return { help: true };
  const command = argv.shift();
  if (!['import', 'watch'].includes(command)) throw new Error('Expected import or watch; use --help');
  const values = {};
  for (let i = 0; i < argv.length; i += 2) {
    if (!['--endpoint', '--source', '--state', '--source-id', '--adapter', '--batch-size', '--interval-ms'].includes(argv[i]) || !argv[i + 1] || argv[i + 1].startsWith('--')) throw new Error('Invalid CLI option; use --help');
    values[argv[i]] = argv[i + 1];
  }
  if (!values['--endpoint'] || !values['--source']) throw new Error('--endpoint and --source are required');
  const url = new URL(values['--endpoint']);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new Error('Endpoint must be HTTP(S) without embedded credentials');
  const positive = (key, fallback) => { const number = Number(values[key] ?? fallback); if (!Number.isSafeInteger(number) || number <= 0) throw new Error(`${key} must be a positive integer`); return number; };
  return { command, endpoint: url.href, source: values['--source'], state: path.resolve(values['--state'] ?? '.agent-logs-collector-state.json'),
    sourceId: values['--source-id'], adapter: values['--adapter'] ?? 'codex', batchSize: positive('--batch-size', 100), intervalMs: positive('--interval-ms', 2000) };
}
async function main(argv) {
  const options = parse([...argv]);
  if (options.help) { process.stdout.write(HELP); return; }
  options.key = process.env.FLOWBASE_AGENT_LOGS_API_KEY;
  if (!options.key) throw new Error('FLOWBASE_AGENT_LOGS_API_KEY is required');
  const controller = new AbortController();
  const stop = () => controller.abort();
  process.once('SIGINT', stop); process.once('SIGTERM', stop);
  try {
    do {
      try { const result = await collect(options); process.stdout.write(`Uploaded ${result.uploaded} events; ${result.unattributed_files} files awaiting source turn identity\n`); }
      catch (error) {
        if (options.command === 'import') throw error;
        process.stderr.write('Collection failed; checkpoint retained; watch will retry\n');
      }
      if (options.command === 'import' || controller.signal.aborted) break;
      try { await delay(options.intervalMs, undefined, { signal: controller.signal }); } catch { break; }
    } while (!controller.signal.aborted);
  } finally { process.removeListener('SIGINT', stop); process.removeListener('SIGTERM', stop); }
}
if (require.main === module) main(process.argv.slice(2)).catch(error => {
  // Adapter/JSON/network errors may contain source data or credentials. Do not echo them.
  process.stderr.write('Agent Logs collection failed. Check options, source format, endpoint and checkpoint ownership.\n'); process.exitCode = 1;
});
module.exports = { parse, main };
