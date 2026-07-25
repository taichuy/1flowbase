'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  manifestDigest, prepareEvidenceRoot, writeClientEvidence, writeConfigManifest, writeJson,
} = require('./evidence');
const { assertNoArtifactSecrets } = require('./artifact-scan');
const {
  claudeEnvironment,
  codexEnvironment,
  opencodeEnvironment,
  sanitizedEnvironment,
} = require('./environment');
const { normalizeInputs } = require('./inputs');
const {
  claudeInvocation,
  codexInvocation,
  opencodeInvocation,
  sanitizedInvocation,
  TEXT_SENTINEL,
  TOOL_SENTINEL,
} = require('./invocations');
const { assertCompatibleResult, executeInvocation, executeTmuxInvocation } = require('./runner');
const { collectClientProvenance } = require('./provenance');
const { readTimeline } = require('./timeline');
const { loadPinnedInventory } = require('../wire-audit/inventory');
const { runWireAudit } = require('../wire-audit/runner');

const DEFAULT_EVIDENCE_ROOT = path.resolve(
  __dirname,
  '../../../../tmp/test-governance/ai-gateway-concurrency/cli-smoke'
);

function temporaryClientPaths(client) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), `1flowbase-${client}-sentinel-`));
  const paths = {
    root,
    home: path.join(root, 'home'),
    config: path.join(root, 'config'),
    output: path.join(root, 'output'),
  };
  for (const directory of [paths.home, paths.config, paths.output]) {
    fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
  }
  return paths;
}

async function runCliSmoke(rawOptions, dependencies = {}) {
  const inputs = normalizeInputs(rawOptions);
  const runId = new Date().toISOString().replaceAll(/[:.]/gu, '-');
  const defaultOutputRoot = rawOptions.tmuxTiming
    ? path.resolve(__dirname, `../../../../tmp/test-governance/compatible-stream-e2e/${runId}`)
    : DEFAULT_EVIDENCE_ROOT;
  const outputRoot = prepareEvidenceRoot(dependencies.outputRoot || rawOptions.evidenceRoot || defaultOutputRoot);
  const fetchImpl = dependencies.fetchImpl || globalThis.fetch;
  const readProducerSnapshot = dependencies.readProducerSnapshot || (inputs.manifest.controlledUpstream
    ? async () => {
      const response = await fetchImpl(inputs.manifest.controlledUpstream.snapshotUrl);
      if (!response.ok) throw new Error(`controlled upstream snapshot returned HTTP ${response.status}`);
      return response.json();
    }
    : undefined);
  const runChild = dependencies.executeInvocation || (rawOptions.tmuxTiming
    ? (invocation, env, client, turn) => executeTmuxInvocation(invocation, env, {
      artifactDirectory: path.join(outputRoot, client, turn),
      markers: turn === 'tool'
        ? ['marker-1', 'marker-2']
        : client === 'opencode' ? ['__unused_text_marker__', TEXT_SENTINEL] : [],
      clientResultMarker: turn === 'tool' ? '1flowbase-client-tool-result' : null,
      readProducerSnapshot: turn === 'tool' ? readProducerSnapshot : undefined,
      secrets,
      onFirstMarker: turn === 'tool'
        ? async () => {
          const response = await fetchImpl(inputs.manifest.controlledUpstream.barrierReleaseUrl, { method: 'POST' });
          if (!response.ok) throw new Error(`barrier release returned HTTP ${response.status}`);
        }
        : undefined,
    })
    : executeInvocation);
  const parentEnv = dependencies.parentEnv || process.env;
  const codexPaths = temporaryClientPaths('codex');
  const claudePaths = temporaryClientPaths('claude');
  const opencodePaths = inputs.opencodeExecutable ? temporaryClientPaths('opencode') : null;
  const secretCanary = rawOptions.secretCanary || 'sk-1flowbase-controlled-secret-canary';
  const secrets = [inputs.manifest.openai.api_key, inputs.manifest.anthropic.api_key, secretCanary];
  for (const paths of [codexPaths, claudePaths, opencodePaths].filter(Boolean)) {
    fs.writeFileSync(
      path.join(paths.output, 'tool-vector.txt'),
      `1flowbase-client-tool-result\n${secretCanary}\n`,
      { mode: 0o600 }
    );
  }
  try {
    const codexPlans = Object.fromEntries(['text', 'tool'].map((turn) => [turn, codexInvocation(
      inputs.codexExecutable, codexPaths, inputs.manifest.gatewayBaseUrl, inputs.manifest.openai, turn
    )]));
    const claudePlans = Object.fromEntries(['text', 'tool'].map((turn) => [turn, claudeInvocation(
      inputs.claudeExecutable, claudePaths, inputs.manifest.anthropic, turn
    )]));
    const opencodePlans = inputs.opencodeExecutable
      ? Object.fromEntries(['text', 'tool'].map((turn) => [turn, opencodeInvocation(
        inputs.opencodeExecutable, opencodePaths, inputs.manifest.openai, turn
      )]))
      : null;
    const provenance = (dependencies.collectClientProvenance || collectClientProvenance)(inputs, {
      codex: codexPlans,
      claude: claudePlans,
      opencode: opencodePlans,
    });
    const codexEnv = codexEnvironment(parentEnv, codexPaths, inputs.manifest.openai.api_key);
    const claudeEnv = claudeEnvironment(
      parentEnv,
      claudePaths,
      inputs.manifest.gatewayBaseUrl,
      inputs.manifest.anthropic.api_key
    );
    const opencodeEnv = opencodePlans
      ? opencodeEnvironment(
        parentEnv,
        opencodePaths,
        inputs.manifest.gatewayBaseUrl,
        inputs.manifest.openai
      )
      : null;
    writeConfigManifest(outputRoot, {
      schema_version: '1flowbase.ai-gateway-cli-smoke-config/v2',
      transport_contract: {
        gateway_role: 'transport-only',
        tool_execution_owner: 'client',
      },
      ready_manifest: {
        path: inputs.manifest.path,
        sha256: manifestDigest(inputs.manifest.path),
        gateway_base_url: inputs.manifest.gatewayBaseUrl,
      },
      targets: {
        codex: {
          application_id: inputs.manifest.openai.application_id,
          model: inputs.manifest.openai.model,
          api_key: '<ephemeral-application-key>',
        },
        claude: {
          application_id: inputs.manifest.anthropic.application_id,
          model: inputs.manifest.anthropic.model,
          api_key: '<ephemeral-application-key>',
        },
        ...(opencodePlans ? {
          opencode: {
            invocations: Object.values(opencodePlans).map(sanitizedInvocation),
            environment: sanitizedEnvironment(opencodeEnv),
          },
        } : {}),
      },
      clients: {
        codex: {
          invocations: Object.values(codexPlans).map(sanitizedInvocation), environment: sanitizedEnvironment(codexEnv),
          provenance: provenance.codex,
        },
        claude: {
          invocations: Object.values(claudePlans).map(sanitizedInvocation), environment: sanitizedEnvironment(claudeEnv),
          provenance: provenance.claude,
        },
        ...(opencodePlans ? {
          opencode: {
            invocations: Object.values(opencodePlans).map(sanitizedInvocation),
            environment: sanitizedEnvironment(opencodeEnv),
            provenance: provenance.opencode,
          },
        } : {}),
      },
    }, secrets);

    const inventory = loadPinnedInventory();
    writeJson(path.join(outputRoot, 'wire-inventory.json'), inventory);
    const wireAudit = !rawOptions.skipWireAudit && inputs.manifest.controlledUpstream
      ? await (dependencies.runWireAudit || runWireAudit)(inputs, {
        fetchImpl,
        secretCanary,
      })
      : null;
    if (wireAudit) writeJson(path.join(outputRoot, 'wire-audit.json'), wireAudit);

    const counts = {};
    for (const [client, plans, env] of [
      ['codex', codexPlans, codexEnv],
      ['claude', claudePlans, claudeEnv],
      ...(opencodePlans ? [['opencode', opencodePlans, opencodeEnv]] : []),
    ]) {
      counts[client] = {};
      for (const turn of ['text', 'tool']) {
        const result = await runChild(plans[turn], env, client, turn);
        writeClientEvidence(outputRoot, client, turn, result, secrets);
        counts[client][turn] = assertCompatibleResult(
          client, result, turn === 'text' ? TEXT_SENTINEL : TOOL_SENTINEL
        );
        if (turn === 'tool' && rawOptions.tmuxTiming) assertMarkerOrder(client, result);
      }
    }

    if (readProducerSnapshot) {
      writeJson(path.join(outputRoot, 'producer-snapshot.json'), await readProducerSnapshot());
    }
    const scannedArtifacts = assertNoArtifactSecrets([outputRoot], secrets);

    return {
      schema_version: '1flowbase.ai-gateway-cli-smoke-result/v1',
      status: 'pass',
      evidence_root: outputRoot,
      event_counts: counts,
      scanned_artifact_count: scannedArtifacts.length,
    };
  } finally {
    fs.rmSync(codexPaths.root, { recursive: true, force: true });
    fs.rmSync(claudePaths.root, { recursive: true, force: true });
    if (opencodePaths) fs.rmSync(opencodePaths.root, { recursive: true, force: true });
  }
}

function assertMarkerOrder(client, result) {
  const events = readTimeline(result.pty?.timeline_path);
  const first = events.findIndex((event) => event.event === 'marker_1');
  const second = events.findIndex((event) => event.event === 'marker_2');
  if (first === -1 || second === -1) {
    throw new Error(`${client} PTY did not expose both streaming markers`);
  }
  if (second <= first) throw new Error(`${client} second streaming marker was not observed after the first`);
  const required = [
    'tool_call', 'client_result', 'second_upstream_request', 'marker_1',
    'barrier_release', 'marker_2', 'terminal',
  ];
  const positions = required.map((event) => events.findIndex((entry) => entry.event === event));
  if (positions.some((position) => position === -1)
    || positions.some((position, index) => index > 0 && position <= positions[index - 1])) {
    throw new Error(`${client} timeline did not prove live producer/client barrier chronology`);
  }
}

module.exports = { DEFAULT_EVIDENCE_ROOT, runCliSmoke, temporaryClientPaths };
