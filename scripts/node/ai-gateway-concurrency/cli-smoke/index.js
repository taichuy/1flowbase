'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { manifestDigest, prepareEvidenceRoot, writeClientEvidence, writeConfigManifest } = require('./evidence');
const { claudeEnvironment, codexEnvironment, sanitizedEnvironment } = require('./environment');
const { normalizeInputs } = require('./inputs');
const { claudeInvocation, codexInvocation, sanitizedInvocation } = require('./invocations');
const { assertCompatibleResult, executeInvocation } = require('./runner');

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
  const outputRoot = prepareEvidenceRoot(dependencies.outputRoot || DEFAULT_EVIDENCE_ROOT);
  const runChild = dependencies.executeInvocation || executeInvocation;
  const parentEnv = dependencies.parentEnv || process.env;
  const codexPaths = temporaryClientPaths('codex');
  const claudePaths = temporaryClientPaths('claude');
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
    const codexEnv = codexEnvironment(parentEnv, codexPaths, inputs.manifest.openai.api_key);
    const claudeEnv = claudeEnvironment(
      parentEnv,
      claudePaths,
      inputs.manifest.gatewayBaseUrl,
      inputs.manifest.anthropic.api_key
    );
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
      },
      clients: {
        codex: { invocation: sanitizedInvocation(codexPlan), environment: sanitizedEnvironment(codexEnv) },
        claude: { invocation: sanitizedInvocation(claudePlan), environment: sanitizedEnvironment(claudeEnv) },
      },
    });

    const codexResult = await runChild(codexPlan, codexEnv);
    writeClientEvidence(outputRoot, 'codex', codexResult, secrets);
    const codexEventCount = assertCompatibleResult('codex', codexResult);

    const claudeResult = await runChild(claudePlan, claudeEnv);
    writeClientEvidence(outputRoot, 'claude', claudeResult, secrets);
    const claudeEventCount = assertCompatibleResult('claude', claudeResult);

    return {
      schema_version: '1flowbase.ai-gateway-cli-smoke-result/v1',
      status: 'pass',
      evidence_root: outputRoot,
      codex_event_count: codexEventCount,
      claude_event_count: claudeEventCount,
    };
  } finally {
    fs.rmSync(codexPaths.root, { recursive: true, force: true });
    fs.rmSync(claudePaths.root, { recursive: true, force: true });
  }
}

module.exports = { DEFAULT_EVIDENCE_ROOT, runCliSmoke, temporaryClientPaths };
