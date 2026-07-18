'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const net = require('node:net');
const os = require('node:os');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');
const { setTimeout: delay } = require('node:timers/promises');

const PAIR_ARTIFACT_SCHEMA = '1flowbase.provider-conformance-pair/v1';
const REQUIRED_PROVIDER_CODES = [
  'openai',
  'anthropic',
  'aliyun_bailian',
  'deepseek',
  'gemini',
  'openai_compatible',
];
const MAX_CAPTURE_BYTES = 64 * 1024;
const HTTP_TIMEOUT_MS = 30_000;

class ConformanceError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ConformanceError';
  }
}

function fail(message) {
  throw new ConformanceError(message);
}

function requireCondition(value, message) {
  if (!value) {
    fail(message);
  }
}

function sha256Bytes(value) {
  return crypto.createHash('sha256').update(value).digest('hex');
}

function sha256File(filePath) {
  return sha256Bytes(fs.readFileSync(filePath));
}

function stableValue(value) {
  if (Array.isArray(value)) {
    return value.map(stableValue);
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, stableValue(value[key])])
    );
  }
  return value;
}

function stableJson(value) {
  return JSON.stringify(stableValue(value));
}

function replaceTokens(value, tokens) {
  if (Array.isArray(value)) {
    return value.map((item) => replaceTokens(item, tokens));
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, replaceTokens(item, tokens)])
    );
  }
  if (typeof value !== 'string') {
    return value;
  }
  return Object.entries(tokens).reduce(
    (result, [token, replacement]) => result.split(token).join(replacement),
    value
  );
}

function runCommand(command, args, { cwd, label }) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: 'utf8',
    maxBuffer: 128 * 1024,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (result.error || result.status !== 0) {
    fail(`${label} failed`);
  }
  return (result.stdout || '').trim();
}

function sourceSnapshot(root, label) {
  requireCondition(fs.existsSync(root), `${label} source root does not exist`);
  const actualSha = runCommand('git', ['rev-parse', 'HEAD'], { cwd: root, label });
  const status = runCommand('git', ['status', '--porcelain=v1', '-z'], {
    cwd: root,
    label,
  });
  return {
    actual_sha: actualSha,
    dirty: status.length > 0,
  };
}

function verifyPairSnapshot({ main, official, packageDigests, expectedPackageDigests }) {
  if (main.actual_sha !== main.expected_sha) {
    fail('main source SHA mismatch');
  }
  if (official.actual_sha !== official.expected_sha) {
    fail('official source SHA mismatch');
  }
  if (main.dirty) {
    fail('main source tree is dirty');
  }
  if (official.dirty) {
    fail('official source tree is dirty');
  }
  if (
    expectedPackageDigests &&
    stableJson(packageDigests || {}) !== stableJson(expectedPackageDigests)
  ) {
    fail('package digest mismatch');
  }
}

function assertPairNegativeFixtures(matrix) {
  const cases = matrix.negative_cases?.pair || [];
  for (const fixture of cases) {
    let caught = null;
    try {
      const packageDigests = fixture.id === 'package-digest-mismatch' ? { openai: 'a' } : {};
      const expectedPackageDigests =
        fixture.id === 'package-digest-mismatch' ? { openai: 'b' } : undefined;
      verifyPairSnapshot({
        main: fixture.main,
        official: fixture.official,
        packageDigests,
        expectedPackageDigests,
      });
    } catch (error) {
      caught = error;
    }
    requireCondition(
      caught instanceof ConformanceError && caught.message === fixture.expected_error,
      `pair negative fixture ${fixture.id} did not reject as expected`
    );
  }
}

function readJson(filePath, label) {
  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  } catch {
    fail(`${label} is not valid JSON`);
  }
}

function normalizeDigestList(packages) {
  return [...packages]
    .map(({ provider_code, sha256 }) => ({ provider_code, sha256 }))
    .sort((left, right) => left.provider_code.localeCompare(right.provider_code));
}

function validateExpectedPairArtifact({ artifactPath, mainSha, officialSha, matrixSha, packages }) {
  if (!artifactPath) {
    return;
  }
  const artifact = readJson(artifactPath, 'expected paired SHA artifact');
  requireCondition(
    artifact.schema_version === PAIR_ARTIFACT_SCHEMA,
    'expected paired SHA artifact schema mismatch'
  );
  requireCondition(artifact.main_sha === mainSha, 'expected paired SHA artifact main SHA mismatch');
  requireCondition(
    artifact.official_sha === officialSha,
    'expected paired SHA artifact official SHA mismatch'
  );
  requireCondition(
    artifact.matrix_sha256 === matrixSha,
    'expected paired SHA artifact matrix digest mismatch'
  );
  if (stableJson(normalizeDigestList(artifact.package_digests || [])) !== stableJson(packages)) {
    fail('package digest mismatch');
  }
}

function parseScalar(raw) {
  return raw.trim().replace(/^['"]|['"]$/g, '');
}

function parseManifestFacts(manifestPath) {
  const lines = fs.readFileSync(manifestPath, 'utf8').split(/\r?\n/);
  const facts = {
    plugin_id: null,
    version: null,
    contract_version: null,
    consumption_kind: null,
    execution_mode: null,
    slot_codes: [],
    runtime: { protocol: null, entry: null, capabilities: [] },
  };
  let list = null;
  let inRuntime = false;
  let runtimeList = null;

  for (const rawLine of lines) {
    if (!rawLine.trim() || rawLine.trimStart().startsWith('#')) {
      continue;
    }
    const indent = rawLine.length - rawLine.trimStart().length;
    const line = rawLine.trim();
    if (indent === 0) {
      list = null;
      runtimeList = null;
      inRuntime = line === 'runtime:';
      if (line === 'slot_codes:') {
        list = 'slot_codes';
        continue;
      }
      const match = /^([a-z_]+):\s*(.*)$/.exec(line);
      if (match && Object.hasOwn(facts, match[1])) {
        facts[match[1]] = parseScalar(match[2]);
      }
      continue;
    }
    if (list === 'slot_codes' && indent === 2 && line.startsWith('- ')) {
      facts.slot_codes.push(parseScalar(line.slice(2)));
      continue;
    }
    if (!inRuntime) {
      continue;
    }
    if (indent === 2) {
      runtimeList = line === 'capabilities:' ? 'capabilities' : null;
      const match = /^([a-z_]+):\s*(.*)$/.exec(line);
      if (match && Object.hasOwn(facts.runtime, match[1])) {
        facts.runtime[match[1]] = parseScalar(match[2]);
      }
      continue;
    }
    if (runtimeList === 'capabilities' && indent === 4 && line.startsWith('- ')) {
      facts.runtime.capabilities.push(parseScalar(line.slice(2)));
    }
  }

  requireCondition(facts.plugin_id, 'actual package manifest is missing plugin_id');
  requireCondition(facts.version, 'actual package manifest is missing version');
  requireCondition(facts.contract_version, 'actual package manifest is missing contract_version');
  return facts;
}

function assertManifest(provider, actual) {
  const expected = provider.expected_manifest;
  const expectedFact = {
    plugin_id: provider.plugin_id,
    contract_version: expected.contract_version,
    consumption_kind: expected.consumption_kind,
    execution_mode: expected.execution_mode,
    slot_codes: [...expected.slot_codes].sort(),
    runtime: {
      protocol: expected.runtime.protocol,
      entry: expected.runtime.entry,
      capabilities: [...expected.runtime.capabilities].sort(),
    },
  };
  const actualFact = {
    plugin_id: actual.plugin_id,
    contract_version: actual.contract_version,
    consumption_kind: actual.consumption_kind,
    execution_mode: actual.execution_mode,
    slot_codes: [...actual.slot_codes].sort(),
    runtime: {
      protocol: actual.runtime.protocol,
      entry: actual.runtime.entry,
      capabilities: [...actual.runtime.capabilities].sort(),
    },
  };
  requireCondition(
    stableJson(actualFact) === stableJson(expectedFact),
    `actual package manifest does not match ${provider.provider_code} fixture`
  );
}

function walkPackageFiles(root) {
  const files = [];
  const visit = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const candidate = path.join(current, entry.name);
      if (entry.isDirectory()) {
        visit(candidate);
      } else if (entry.isFile() && entry.name.endsWith('.1flowbasepkg')) {
        files.push(candidate);
      }
    }
  };
  visit(root);
  return files.sort();
}

function extractPackage(packagePath, destination) {
  fs.mkdirSync(destination, { recursive: true });
  runCommand('tar', ['-xzf', packagePath, '-C', destination], {
    label: 'actual provider package extraction',
  });
}

function discoverActualPackages(packageDir, matrix, scratchRoot) {
  requireCondition(fs.existsSync(packageDir), 'actual package directory does not exist');
  const files = walkPackageFiles(packageDir);
  requireCondition(files.length > 0, 'actual package directory contains no .1flowbasepkg files');
  const providersByCode = new Map(matrix.providers.map((provider) => [provider.provider_code, provider]));
  const discovered = new Map();

  for (const [index, packagePath] of files.entries()) {
    const extractionRoot = path.join(scratchRoot, `package-${index}`);
    extractPackage(packagePath, extractionRoot);
    const manifestPath = path.join(extractionRoot, 'manifest.yaml');
    requireCondition(fs.existsSync(manifestPath), 'actual package is missing manifest.yaml');
    const manifest = parseManifestFacts(manifestPath);
    const provider = providersByCode.get(manifest.plugin_id);
    requireCondition(provider, 'actual package has an unexpected provider identity');
    requireCondition(!discovered.has(provider.provider_code), 'actual package provider is duplicated');
    assertManifest(provider, manifest);
    discovered.set(provider.provider_code, {
      provider,
      package_path: packagePath,
      package_root: extractionRoot,
      manifest,
      sha256: sha256File(packagePath),
    });
  }

  requireCondition(
    stableJson([...discovered.keys()].sort()) === stableJson(REQUIRED_PROVIDER_CODES.slice().sort()),
    'actual package set does not contain the fixed six-provider matrix'
  );
  return discovered;
}

function createBoundedCapture(stream) {
  let text = '';
  stream.on('data', (chunk) => {
    text = `${text}${chunk.toString('utf8')}`;
    if (Buffer.byteLength(text, 'utf8') > MAX_CAPTURE_BYTES) {
      text = text.slice(-MAX_CAPTURE_BYTES);
    }
  });
  return {
    clear() {
      text = '';
    },
    snapshot() {
      return text;
    },
  };
}

function reserveLoopbackPort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close((error) => (error ? reject(error) : resolve(port)));
    });
  });
}

function requestJson(url, method, payload) {
  return new Promise((resolve, reject) => {
    const hasPayload = payload !== null && payload !== undefined;
    const encoded = hasPayload ? Buffer.from(JSON.stringify(payload)) : null;
    const headers = hasPayload
      ? {
          accept: 'application/json',
          'content-type': 'application/json',
          'content-length': encoded.length,
        }
      : { accept: 'application/json' };
    const request = http.request(
      url,
      {
        method,
        headers,
        timeout: HTTP_TIMEOUT_MS,
      },
      (response) => {
        const chunks = [];
        let size = 0;
        response.on('data', (chunk) => {
          size += chunk.length;
          if (size <= 256 * 1024) {
            chunks.push(chunk);
          }
        });
        response.on('end', () => {
          if (size > 256 * 1024) {
            reject(new ConformanceError('plugin runner response exceeded the safe bound'));
            return;
          }
          const body = Buffer.concat(chunks).toString('utf8');
          let parsed = null;
          try {
            parsed = body ? JSON.parse(body) : null;
          } catch {
            reject(new ConformanceError('plugin runner returned invalid JSON'));
            return;
          }
          resolve({ status: response.statusCode || 0, body: parsed });
        });
      }
    );
    request.once('timeout', () => request.destroy(new Error('timeout')));
    request.once('error', () => reject(new ConformanceError('plugin runner HTTP request failed')));
    request.end(encoded || undefined);
  });
}

async function waitForHealth(baseUrl) {
  const deadline = Date.now() + HTTP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    try {
      const result = await requestJson(`${baseUrl}/health`, 'GET', null);
      if (result.status === 200 && result.body?.service === 'plugin-runner') {
        return;
      }
    } catch {
      // The binary may still be binding its loopback listener.
    }
    await delay(100);
  }
  fail('plugin runner did not become healthy');
}

async function startPluginRunner(binaryPath) {
  requireCondition(fs.existsSync(binaryPath), 'plugin runner binary does not exist');
  const port = await reserveLoopbackPort();
  const processHandle = spawn(binaryPath, [], {
    cwd: path.dirname(binaryPath),
    env: {
      ...process.env,
      PLUGIN_RUNNER_ADDR: `127.0.0.1:${port}`,
      RUST_LOG: 'info',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const stdout = createBoundedCapture(processHandle.stdout);
  const stderr = createBoundedCapture(processHandle.stderr);
  const baseUrl = `http://127.0.0.1:${port}`;
  try {
    await waitForHealth(baseUrl);
  } catch (error) {
    processHandle.kill('SIGKILL');
    throw error;
  }
  return {
    base_url: baseUrl,
    output: {
      clear() {
        stdout.clear();
        stderr.clear();
      },
      snapshot() {
        return `${stdout.snapshot()}${stderr.snapshot()}`;
      },
    },
    async stop() {
      if (processHandle.exitCode !== null || processHandle.killed) {
        return;
      }
      processHandle.kill('SIGTERM');
      await Promise.race([
        new Promise((resolve) => processHandle.once('exit', resolve)),
        delay(2_000),
      ]);
      if (processHandle.exitCode === null && !processHandle.killed) {
        processHandle.kill('SIGKILL');
      }
    },
  };
}

function readIncomingBody(request) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    request.on('data', (chunk) => {
      size += chunk.length;
      if (size <= 1024 * 1024) {
        chunks.push(chunk);
      }
    });
    request.on('end', () => {
      if (size > 1024 * 1024) {
        reject(new ConformanceError('fake upstream request exceeded the safe bound'));
        return;
      }
      resolve(Buffer.concat(chunks).toString('utf8'));
    });
    request.on('error', () => reject(new ConformanceError('fake upstream request read failed')));
  });
}

function sse(events) {
  return events
    .map((event) => `data: ${event === '[DONE]' ? event : JSON.stringify(event)}\n\n`)
    .join('');
}

function fakeResponseBody(kind) {
  switch (kind) {
    case 'openai_responses_sse':
      return sse([
        { type: 'response.created', response: { id: 'resp_conformance' } },
        { type: 'response.output_text.delta', delta: 'conformance-complete' },
        {
          type: 'response.completed',
          response: {
            id: 'resp_conformance',
            status: 'completed',
            usage: { input_tokens: 1, output_tokens: 2, total_tokens: 3 },
          },
        },
      ]);
    case 'anthropic_messages_sse':
      return sse([
        { type: 'message_start', message: { id: 'msg_conformance', usage: { input_tokens: 1 } } },
        {
          type: 'content_block_delta',
          index: 0,
          delta: { type: 'text_delta', text: 'conformance-complete' },
        },
        {
          type: 'message_delta',
          delta: { stop_reason: 'end_turn' },
          usage: { output_tokens: 2 },
        },
        '[DONE]',
      ]);
    case 'openai_chat_sse':
      return sse([
        {
          id: 'chatcmpl_conformance',
          model: 'conformance-model',
          choices: [{ delta: { content: 'conformance-complete' }, finish_reason: null }],
        },
        {
          id: 'chatcmpl_conformance',
          choices: [{ delta: {}, finish_reason: 'stop' }],
          usage: { prompt_tokens: 1, completion_tokens: 2, total_tokens: 3 },
        },
        '[DONE]',
      ]);
    case 'gemini_generate_content_sse':
      return sse([
        {
          responseId: 'gemini_conformance',
          modelVersion: 'conformance-model',
          candidates: [
            {
              content: { parts: [{ text: 'conformance-complete' }] },
              finishReason: 'STOP',
            },
          ],
          usageMetadata: {
            promptTokenCount: 1,
            candidatesTokenCount: 2,
            totalTokenCount: 3,
          },
        },
      ]);
    default:
      fail('fixture requests an unknown fake upstream response');
  }
}

async function createFakeUpstream() {
  const requests = [];
  let activeResponse = null;
  const server = http.createServer(async (request, response) => {
    try {
      const rawBody = await readIncomingBody(request);
      let body;
      try {
        body = rawBody ? JSON.parse(rawBody) : null;
      } catch {
        response.writeHead(400, { 'content-type': 'application/json' });
        response.end('{"error":"invalid JSON"}');
        return;
      }
      const headers = Object.fromEntries(
        Object.entries(request.headers).map(([name, value]) => [name.toLowerCase(), String(value)])
      );
      requests.push({ method: request.method || '', path: request.url || '', headers, body });
      const payload = fakeResponseBody(activeResponse);
      response.writeHead(200, {
        'content-type': 'text/event-stream',
        'content-length': Buffer.byteLength(payload),
        connection: 'close',
      });
      response.end(payload);
    } catch {
      response.writeHead(500, { 'content-type': 'application/json' });
      response.end('{"error":"fake upstream failure"}');
    }
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const { port } = server.address();
  return {
    base_url: `http://127.0.0.1:${port}`,
    begin(responseFixture) {
      activeResponse = responseFixture;
      return requests.length;
    },
    countSince(index) {
      return requests.length - index;
    },
    takeOnlySince(index) {
      const selected = requests.slice(index);
      requests.splice(index, selected.length);
      return selected;
    },
    async close() {
      await new Promise((resolve) => server.close(resolve));
    },
  };
}

function assertWire(provider, captured, tokens) {
  requireCondition(captured.length === 1, `fake upstream request count failed for ${provider.provider_code}`);
  const request = captured[0];
  const expected = replaceTokens(provider.expected_wire, tokens);
  requireCondition(
    request.method === expected.method && request.path === expected.path,
    `vendor method or path mismatch for ${provider.provider_code}`
  );
  for (const [name, value] of Object.entries(expected.headers)) {
    requireCondition(
      request.headers[name] === value,
      `vendor header mismatch for ${provider.provider_code}`
    );
  }
  requireCondition(
    stableJson(request.body) === stableJson(expected.body),
    `vendor request body mismatch for ${provider.provider_code}`
  );
}

async function assertWireAudit(runner, provider, tokens) {
  const expected = provider.expected_wire.wire_audit;
  const deadline = Date.now() + 1_500;
  let output = '';
  while (Date.now() < deadline) {
    output = runner.output.snapshot();
    if (output.includes('provider generate wire prepared')) {
      break;
    }
    await delay(25);
  }
  requireCondition(output.includes('provider generate wire prepared'), 'WireAudit was not emitted');
  for (const [field, value] of Object.entries(expected)) {
    requireCondition(
      output.includes(`${field}: ${String(value)}`),
      `WireAudit field mismatch for ${provider.provider_code}`
    );
  }
  for (const sensitiveValue of Object.values(tokens)) {
    requireCondition(
      !output.includes(sensitiveValue),
      'WireAudit leaked a raw or secret canary'
    );
  }
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

async function loadPackage(runner, packageInfo) {
  const loaded = await requestJson(`${runner.base_url}/providers/load`, 'POST', {
    package_root: packageInfo.package_root,
  });
  requireCondition(loaded.status === 200, 'plugin runner rejected an actual package');
  requireCondition(
    loaded.body?.provider_code === packageInfo.provider.provider_code,
    'plugin runner loaded the wrong actual package'
  );
  return loaded.body.plugin_id;
}

async function invokePackage(runner, pluginId, input) {
  return requestJson(`${runner.base_url}/providers/invoke-stream`, 'POST', {
    plugin_id: pluginId,
    input,
  });
}

async function runNoSpawnNegatives({ runner, upstream, packageInfo, matrix, tokens }) {
  const normalInput = replaceTokens(cloneJson(packageInfo.provider.input), {
    ...tokens,
    $UPSTREAM_BASE_URL: upstream.base_url,
  });
  const pluginId = packageInfo.loaded_plugin_id;

  const legacy = matrix.negative_cases.legacy_input;
  runner.output.clear();
  const legacyStart = upstream.begin(packageInfo.provider.response_fixture);
  const legacyInput = cloneJson(normalInput);
  legacyInput.contract_version = legacy.contract_version;
  const legacyResult = await invokePackage(runner, pluginId, legacyInput);
  requireCondition(
    Math.floor(legacyResult.status / 100) === legacy.expected_status_family,
    'legacy provider input did not fail at the host boundary'
  );
  requireCondition(
    upstream.countSince(legacyStart) === legacy.expected_upstream_requests,
    'legacy provider input reached the upstream'
  );
  requireCondition(
    !runner.output.snapshot().includes('provider generate wire prepared'),
    'legacy provider input prepared a provider wire'
  );

  const capability = matrix.negative_cases.undeclared_capability;
  runner.output.clear();
  const capabilityStart = upstream.begin(packageInfo.provider.response_fixture);
  const capabilityInput = cloneJson(normalInput);
  capabilityInput.required_capabilities = capability.required_capabilities;
  capabilityInput.system = replaceTokens(capability.system, tokens);
  const capabilityResult = await invokePackage(runner, pluginId, capabilityInput);
  requireCondition(
    Math.floor(capabilityResult.status / 100) === capability.expected_status_family,
    'undeclared capability input did not fail at the host boundary'
  );
  requireCondition(
    upstream.countSince(capabilityStart) === capability.expected_upstream_requests,
    'undeclared capability input reached the upstream'
  );
  requireCondition(
    !runner.output.snapshot().includes('provider generate wire prepared'),
    'undeclared capability input prepared a provider wire'
  );
}

function makeCanaries() {
  const nonce = crypto.randomBytes(18).toString('hex');
  return {
    $PROMPT_CANARY: `prompt-canary-${nonce}`,
    $SYSTEM_PROMPT_CANARY: `system-canary-${nonce}`,
    $HEADER_CANARY: `header-canary-${nonce}`,
    $SECRET_CANARY: `secret-canary-${nonce}`,
    $END_USER_CANARY: `end-user-canary-${nonce}`,
  };
}

function writePairArtifact({ artifactPath, mainSha, officialSha, matrixSha, packages, providers, tokens }) {
  const artifact = {
    schema_version: PAIR_ARTIFACT_SCHEMA,
    main_sha: mainSha,
    official_sha: officialSha,
    matrix_sha256: matrixSha,
    matrix_provider_codes: REQUIRED_PROVIDER_CODES,
    package_digests: normalizeDigestList(packages),
    providers: providers
      .map(({ provider_code, plugin_id, plugin_version }) => ({
        provider_code,
        plugin_id,
        plugin_version,
      }))
      .sort((left, right) => left.provider_code.localeCompare(right.provider_code)),
  };
  const encoded = `${JSON.stringify(artifact, null, 2)}\n`;
  for (const sensitiveValue of Object.values(tokens)) {
    requireCondition(!encoded.includes(sensitiveValue), 'paired SHA artifact contains a sensitive canary');
  }
  fs.mkdirSync(path.dirname(artifactPath), { recursive: true });
  fs.writeFileSync(artifactPath, encoded, { mode: 0o600 });
}

async function runConformance(options) {
  const matrix = readJson(options.fixture, 'provider conformance matrix');
  requireCondition(
    matrix.schema_version === '1flowbase.provider-conformance/v1',
    'provider conformance matrix schema mismatch'
  );
  requireCondition(
    stableJson(matrix.providers.map((provider) => provider.provider_code).sort()) ===
      stableJson(REQUIRED_PROVIDER_CODES.slice().sort()),
    'provider conformance matrix is not the fixed six-provider matrix'
  );
  assertPairNegativeFixtures(matrix);

  const matrixSha = sha256File(options.fixture);
  const scratchRoot = fs.mkdtempSync(path.join(os.tmpdir(), '1flowbase-provider-conformance-'));
  let runner = null;
  let upstream = null;
  const canaries = makeCanaries();

  try {
    const main = sourceSnapshot(options.mainRoot, 'main source');
    const official = sourceSnapshot(options.officialRoot, 'official source');
    verifyPairSnapshot({
      main: { ...main, expected_sha: options.mainSha },
      official: { ...official, expected_sha: options.officialSha },
    });
    const packages = discoverActualPackages(options.packageDir, matrix, scratchRoot);
    const packageDigests = [...packages.values()].map((item) => ({
      provider_code: item.provider.provider_code,
      sha256: item.sha256,
    }));
    validateExpectedPairArtifact({
      artifactPath: options.expectedPairArtifact,
      mainSha: options.mainSha,
      officialSha: options.officialSha,
      matrixSha,
      packages: normalizeDigestList(packageDigests),
    });

    runner = await startPluginRunner(options.pluginRunnerBin);
    upstream = await createFakeUpstream();
    for (const providerCode of REQUIRED_PROVIDER_CODES) {
      const packageInfo = packages.get(providerCode);
      packageInfo.loaded_plugin_id = await loadPackage(runner, packageInfo);
    }

    await runNoSpawnNegatives({
      runner,
      upstream,
      packageInfo: packages.get(matrix.negative_cases.legacy_input.target_provider),
      matrix,
      tokens: canaries,
    });

    for (const providerCode of REQUIRED_PROVIDER_CODES) {
      const packageInfo = packages.get(providerCode);
      runner.output.clear();
      const requestStart = upstream.begin(packageInfo.provider.response_fixture);
      const input = replaceTokens(cloneJson(packageInfo.provider.input), {
        ...canaries,
        $UPSTREAM_BASE_URL: upstream.base_url,
      });
      const result = await invokePackage(runner, packageInfo.loaded_plugin_id, input);
      requireCondition(result.status === 200, `actual package invocation failed for ${providerCode}`);
      requireCondition(
        result.body?.result?.final_content === 'conformance-complete',
        `actual package result was not produced for ${providerCode}`
      );
      assertWire(packageInfo.provider, upstream.takeOnlySince(requestStart), canaries);
      await assertWireAudit(runner, packageInfo.provider, canaries);
    }

    writePairArtifact({
      artifactPath: options.artifact,
      mainSha: options.mainSha,
      officialSha: options.officialSha,
      matrixSha,
      packages: packageDigests,
      providers: [...packages.values()].map((item) => ({
        provider_code: item.provider.provider_code,
        plugin_id: item.manifest.plugin_id,
        plugin_version: item.manifest.version,
      })),
      tokens: canaries,
    });
    return {
      main_sha: options.mainSha,
      official_sha: options.officialSha,
      matrix_sha256: matrixSha,
      package_count: packageDigests.length,
    };
  } finally {
    if (upstream) {
      await upstream.close();
    }
    if (runner) {
      await runner.stop();
    }
    fs.rmSync(scratchRoot, { recursive: true, force: true });
  }
}

module.exports = {
  ConformanceError,
  PAIR_ARTIFACT_SCHEMA,
  REQUIRED_PROVIDER_CODES,
  runConformance,
  stableJson,
  verifyPairSnapshot,
};
