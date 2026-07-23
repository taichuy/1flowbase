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
      schema_version: '1flowbase.ai-gateway-cli-smoke-config/v1',
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
        codex: { invocation: sanitizedInvocation(codexPlan), environment: sanitizedEnvironment(codexEnv) },
        claude: { invocation: sanitizedInvocation(claudePlan), environment: sanitizedEnvironment(claudeEnv) },
      },
    });

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
  const first = result.pty?.markers?.[options.firstMarker];
  const second = result.pty?.markers?.[options.secondMarker];
  if (typeof first !== 'number' || typeof second !== 'number') {
    throw new Error(`${client} PTY did not expose both streaming markers`);
  }
  if (second <= first) throw new Error(`${client} second streaming marker was not observed after the first`);
}

module.exports = { DEFAULT_EVIDENCE_ROOT, runCliSmoke, temporaryClientPaths };
