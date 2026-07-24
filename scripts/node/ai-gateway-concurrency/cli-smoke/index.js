'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { manifestDigest, prepareEvidenceRoot, writeClientEvidence, writeConfigManifest } = require('./evidence');
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
} = require('./invocations');
const { assertCompatibleResult, executeInvocation, executeTmuxInvocation } = require('./runner');
const { collectClientProvenance } = require('./provenance');
const { readTimeline } = require('./timeline');

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
  const runChild = dependencies.executeInvocation || (rawOptions.tmuxTiming
    ? (invocation, env, client) => executeTmuxInvocation(invocation, env, {
      artifactDirectory: path.join(outputRoot, client),
      markers: [rawOptions.firstMarker, rawOptions.secondMarker].filter(Boolean),
      clientResultMarker: rawOptions.clientResultMarker,
      producerTimelinePath: inputs.producerTimelineDirectory
        ? path.join(inputs.producerTimelineDirectory, `${client}.jsonl`)
        : null,
      secrets,
      onFirstMarker: inputs.barrierReleaseUrl
        ? async () => {
          const response = await fetch(inputs.barrierReleaseUrl, { method: 'POST' });
          if (!response.ok) throw new Error(`barrier release returned HTTP ${response.status}`);
        }
        : undefined,
    })
    : executeInvocation);
  const parentEnv = dependencies.parentEnv || process.env;
  const codexPaths = temporaryClientPaths('codex');
  const claudePaths = temporaryClientPaths('claude');
  const opencodePaths = inputs.opencodeExecutable ? temporaryClientPaths('opencode') : null;
  const secrets = [inputs.manifest.openai.api_key, inputs.manifest.anthropic.api_key];
  try {
    const codexPlan = codexInvocation(
      inputs.codexExecutable,
      codexPaths,
      inputs.manifest.gatewayBaseUrl,
      inputs.manifest.openai
    );
    const claudePlan = claudeInvocation(
      inputs.claudeExecutable,
      claudePaths,
      inputs.manifest.anthropic
    );
    const opencodePlan = inputs.opencodeExecutable
      ? opencodeInvocation(inputs.opencodeExecutable, opencodePaths, inputs.manifest.openai)
      : null;
    const provenance = (dependencies.collectClientProvenance || collectClientProvenance)(inputs, {
      codex: codexPlan,
      claude: claudePlan,
      opencode: opencodePlan,
    });
    const codexEnv = codexEnvironment(parentEnv, codexPaths, inputs.manifest.openai.api_key);
    const claudeEnv = claudeEnvironment(
      parentEnv,
      claudePaths,
      inputs.manifest.gatewayBaseUrl,
      inputs.manifest.anthropic.api_key
    );
    const opencodeEnv = opencodePlan
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
        ...(opencodePlan ? {
          opencode: {
            invocation: sanitizedInvocation(opencodePlan),
            environment: sanitizedEnvironment(opencodeEnv),
          },
        } : {}),
      },
      clients: {
        codex: {
          invocation: sanitizedInvocation(codexPlan), environment: sanitizedEnvironment(codexEnv),
          provenance: provenance.codex,
        },
        claude: {
          invocation: sanitizedInvocation(claudePlan), environment: sanitizedEnvironment(claudeEnv),
          provenance: provenance.claude,
        },
        ...(opencodePlan ? {
          opencode: {
            invocation: sanitizedInvocation(opencodePlan),
            environment: sanitizedEnvironment(opencodeEnv),
            provenance: provenance.opencode,
          },
        } : {}),
      },
    }, secrets);

    const codexResult = await runChild(codexPlan, codexEnv, 'codex');
    writeClientEvidence(outputRoot, 'codex', codexResult, secrets);
    const codexEventCount = assertCompatibleResult('codex', codexResult);
    assertMarkerOrder('codex', codexResult, rawOptions);

    const claudeResult = await runChild(claudePlan, claudeEnv, 'claude');
    writeClientEvidence(outputRoot, 'claude', claudeResult, secrets);
    const claudeEventCount = assertCompatibleResult('claude', claudeResult);
    assertMarkerOrder('claude', claudeResult, rawOptions);

    let opencodeEventCount = null;
    if (opencodePlan) {
      const opencodeResult = await runChild(opencodePlan, opencodeEnv, 'opencode');
      writeClientEvidence(outputRoot, 'opencode', opencodeResult, secrets);
      opencodeEventCount = assertCompatibleResult('opencode', opencodeResult);
      assertMarkerOrder('opencode', opencodeResult, rawOptions);
    }

    return {
      schema_version: '1flowbase.ai-gateway-cli-smoke-result/v1',
      status: 'pass',
      evidence_root: outputRoot,
      codex_event_count: codexEventCount,
      claude_event_count: claudeEventCount,
      opencode_event_count: opencodeEventCount,
    };
  } finally {
    fs.rmSync(codexPaths.root, { recursive: true, force: true });
    fs.rmSync(claudePaths.root, { recursive: true, force: true });
    if (opencodePaths) fs.rmSync(opencodePaths.root, { recursive: true, force: true });
  }
}

function assertMarkerOrder(client, result, options) {
  if (!options.firstMarker && !options.secondMarker) return;
  if (!options.tmuxTiming || !options.firstMarker || !options.secondMarker) {
    throw new Error('first and second markers require --tmux-timing together');
  }
  const events = readTimeline(result.pty?.timeline_path);
  const first = events.findIndex((event) => event.event === 'marker_1');
  const second = events.findIndex((event) => event.event === 'marker_2');
  if (first === -1 || second === -1) {
    throw new Error(`${client} PTY did not expose both streaming markers`);
  }
  if (second <= first) throw new Error(`${client} second streaming marker was not observed after the first`);
  if (options.clientResultMarker && options.producerTimelineDirectory) {
    const required = [
      'tool_call', 'client_result', 'second_upstream_request', 'marker_1',
      'barrier_release', 'marker_2', 'terminal',
    ];
    const positions = required.map((event) => events.findIndex((entry) => entry.event === event));
    if (positions.some((position) => position === -1)
      || positions.some((position, index) => index > 0 && position <= positions[index - 1])) {
      throw new Error(`${client} timeline did not prove producer/client barrier chronology`);
    }
  }
}

module.exports = { DEFAULT_EVIDENCE_ROOT, runCliSmoke, temporaryClientPaths };
