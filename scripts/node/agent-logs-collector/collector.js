'use strict';
const fsp = require('node:fs/promises');
const path = require('node:path');
const crypto = require('node:crypto');
const { files, lines, hash } = require('./source');
const { loadAdapter } = require('./loader');
const SCHEMA = '1flowbase.agent-logs/v1';
async function saveState(file, state) {
  await fsp.mkdir(path.dirname(file), { recursive: true });
  const temp = `${file}.${process.pid}.tmp`;
  const handle = await fsp.open(temp, 'w', 0o600);
  try { await handle.writeFile(JSON.stringify(state)); await handle.sync(); }
  finally { await handle.close(); }
  await fsp.rename(temp, file);
  const directory = await fsp.open(path.dirname(file), 'r');
  try { await directory.sync(); } finally { await directory.close(); }
}
async function upload(endpoint, key, envelope) {
  let response;
  try {
    response = await fetch(endpoint, { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${key}` },
      body: JSON.stringify(envelope), signal: AbortSignal.timeout(30000), redirect: 'error' });
  } catch { throw new Error('Upload failed; checkpoint retained'); }
  if (!response.ok) throw new Error(`Upload rejected (HTTP ${response.status}); checkpoint retained`);
  let receipt;
  try { receipt = (await response.json())?.data; } catch { throw new Error('Invalid upload receipt; checkpoint retained'); }
  // The canonical API returns ApiSuccess<AgentLogsReceipt>: { data, meta }.
  // HTTP success alone is not a durable complete-batch acknowledgment.
  if (!receipt || typeof receipt !== 'object' || Array.isArray(receipt) ||
    !Number.isSafeInteger(receipt.accepted_events) || receipt.accepted_events < 0 ||
    !Number.isSafeInteger(receipt.duplicate_events) || receipt.duplicate_events < 0 ||
    receipt.accepted_events + receipt.duplicate_events !== envelope.events.length ||
    !Array.isArray(receipt.record_ids) || !receipt.record_ids.every(id => typeof id === 'string')) {
    throw new Error('Incomplete upload receipt; checkpoint retained');
  }
  return receipt;
}
async function collect(options) {
  const adapter = loadAdapter(options.adapter);
  const statePath = path.resolve(options.state);
  await fsp.mkdir(path.dirname(statePath), { recursive: true });
  // One collector owns a checkpoint. Never silently steal a potentially active lock.
  let lock;
  try { lock = await fsp.open(`${statePath}.lock`, 'wx', 0o600); }
  catch (error) { if (error.code === 'EEXIST') throw new Error('Checkpoint is locked; after confirming no collector is active, remove its .lock file'); throw error; }
  try {
    await lock.writeFile(String(process.pid));
    let state;
    try { state = JSON.parse(await fsp.readFile(statePath, 'utf8')); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    state ??= { version: 1, source_id: options.sourceId ?? crypto.randomUUID(), source_client: adapter.sourceClient,
      endpoint: options.endpoint, files: {} };
    if (state.version !== 1 || state.source_client !== adapter.sourceClient || state.endpoint !== options.endpoint ||
      (options.sourceId && options.sourceId !== state.source_id)) throw new Error('Checkpoint identity/endpoint differs; use a separate state file');
    // Save installation identity before any HTTP request, including first-request failure.
    await saveState(statePath, state);
    let uploaded = 0, unattributedFiles = 0;
    for await (const file of files(path.resolve(options.source))) {
      let identity, context, checkpoint, verified = false, digest = crypto.createHash('sha256'), batch = [], pending = [];
      let lastPrefix;
      const flush = async () => {
        if (!batch.length) return;
        await upload(options.endpoint, options.key, { schema_version: SCHEMA, source_id: state.source_id,
          source_client: adapter.sourceClient, events: batch.map(entry => entry.event) });
        const acknowledged = batch.at(-1);
        state.files[identity] = { offset: acknowledged.end, prefix_hash: acknowledged.prefix };
        await saveState(statePath, state);
        uploaded += batch.length; batch = [];
      };
      for await (const position of lines(file)) {
        if (!identity) {
          context = adapter.createContext(position.line);
          // Immutable rollout header distinguishes reverted segments and is independent of directories.
          identity = hash(Buffer.concat([Buffer.from(`${adapter.sourceClient}:`), position.bytes]));
          checkpoint = state.files[identity] ?? { offset: 0 };
          verified = checkpoint.offset === 0;
        }
        digest.update(position.bytes); digest.update(String(position.end));
        lastPrefix = digest.copy().digest('hex');
        const event = adapter.convert(position.line, context, position);
        if (position.end <= checkpoint.offset) {
          if (position.end === checkpoint.offset) {
            if (lastPrefix !== checkpoint.prefix_hash) throw new Error('Acknowledged source prefix changed; checkpoint retained');
            verified = true;
          }
          continue;
        }
        if (!verified) throw new Error('Acknowledged source prefix changed or truncated; checkpoint retained');
        if (event == null) continue;
        const entry = { event: { ...event, event_id: hash(`${identity}:${position.start}`), sequence: position.start + 1 },
          end: position.end, prefix: lastPrefix };
        if (typeof event.source_task_id !== 'string' || !event.source_task_id) {
          pending.push(entry); continue;
        }
        // Session-level/pre-turn facts wait for source-declared ownership. Their
        // event identity remains byte-based when projection ownership becomes known.
        for (const buffered of pending) {
          buffered.event.source_task_id = event.source_task_id;
          buffered.event.parent_source_task_id = event.parent_source_task_id;
          batch.push(buffered);
          if (batch.length >= (options.batchSize ?? 100)) await flush();
        }
        pending = [];
        batch.push(entry);
        if (batch.length >= (options.batchSize ?? 100)) await flush();
      }
      if (identity && !verified) throw new Error('Acknowledged source was truncated; checkpoint retained');
      await flush();
      if (pending.length) unattributedFiles++;
    }
    return { uploaded, unattributed_files: unattributedFiles, source_id: state.source_id };
  } finally {
    await lock.close(); await fsp.unlink(`${statePath}.lock`);
  }
}
module.exports = { collect, upload, saveState, SCHEMA };
