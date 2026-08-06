const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const { getRepoRoot } = require('../testing/warning-capture.js');

const OUTPUT_ROOT = path.join('tmp', 'test-governance', 'foundation-contracts');
const FOUNDATION_IDS = [
  'ai-gateway',
  'mcp-gateway',
  'application-backend',
  'native-react',
];
const MCP_CORE_OPERATIONS = ['mcp.list', 'mcp.get', 'mcp.call'];

const FOUNDATION_DEFINITIONS = {
  'ai-gateway': {
    risk: 'protocol projection, transport compatibility, and provider-facing contract drift',
    matches(filePath) {
      return /^scripts\/node\/(?:ai-gateway-concurrency|provider-conformance|verify-state-protocols)(?:\/|\.js$)/u.test(filePath)
        || /^scripts\/node\/cli\/(?:ai-gateway|acp-claude-smoke)/u.test(filePath)
        || /^api\/apps\/api-server\/src\/routes\/application_public_api\/(?:anthropic\.rs|compat_sse(?:\/|\.rs$))/u.test(filePath)
        || /^\.github\/workflows\/ai-gateway-concurrency\.yml$/u.test(filePath);
    },
    fast: [
      {
        id: 'ai-gateway-fast-protocol',
        command: 'node',
        args: ['scripts/node/test-scripts.js', 'ai-gateway-concurrency', 'verify-state-protocols'],
        cwd: '.',
      },
    ],
    full: [
      'nightly/manual: official provider packages, concurrency, complete transport matrix, paired source SHA',
    ],
  },
  'mcp-gateway': {
    risk: 'tool discovery, description, invocation, ACL, mapping, and continuation drift',
    matches(filePath) {
      return /^api\/(?:apps|crates)\/.*(?:\/|_)(?:mcp)(?:\/|_|\.|-)/iu.test(filePath)
        || /^scripts\/node\/(?:export-mcp-instance-to-official|mcp-)/u.test(filePath);
    },
    fast: [
      {
        id: 'mcp-core-list-get-call',
        command: 'cargo',
        args: ['test', '-p', 'api-server', 'mcp_protocol_routes'],
        cwd: 'api',
      },
    ],
    full: [
      'nightly/manual: MCP bundle, upstream MCP, storage, mapping, and large-result matrix',
    ],
  },
  'application-backend': {
    risk: 'data model definition, physical schema, runtime API, scope/ACL, and metadata preservation drift',
    matches(filePath) {
      return /^api\/crates\/control-plane\/src\/(?:model_definition|_tests\/model_definition)/u.test(filePath)
        || /^api\/crates\/storage-durable\/postgres\/(?:migrations\/.*model_definition|src\/(?:model_definition_repository|mappers\/model_definition_mapper)|src\/_tests\/model_definition)/u.test(filePath)
        || /^api\/apps\/api-server\/src\/(?:_tests\/application\/model_definition_routes|routes\/plugins_and_models\/(?:model_definitions|runtime_models|data_sources)|openapi(?:_interface)?\/)/u.test(filePath)
        || /^api\/crates\/runtime-core\/src\/(?:runtime_model_registry|runtime_record_repository|runtime_acl|model_metadata|_tests\/(?:runtime_model_registry_tests|runtime_acl_tests))\.rs$/u.test(filePath);
    },
    fast: [
      {
        id: 'application-backend-model-definition-api',
        command: 'cargo',
        args: ['test', '-p', 'api-server', 'model_definition_routes_'],
        cwd: 'api',
      },
    ],
    full: [
      'nightly: migration/reconcile replay, physical schema, runtime CRUD/OpenAPI, scope/ACL, metadata preservation, coverage',
    ],
  },
  'native-react': {
    risk: 'standard Component source, compiler/runtime ABI, dependency lock, capability, ShadowRoot, and stale artifact drift',
    matches(filePath) {
      if (/(?:^|\/)(?:i18n|locales)(?:\/|$)/u.test(filePath) || /\.css$/u.test(filePath)) {
        return false;
      }
      return /^web\/packages\/(?:page-runtime|block-sdk)\/src\/.*\.(?:ts|tsx)$/u.test(filePath)
        || /^web\/app\/src\/(?:features\/frontstage|shared\/code-block)\/.*\.(?:ts|tsx)$/u.test(filePath)
        || /^web\/(?:app\/package\.json|pnpm-lock\.yaml)$/u.test(filePath)
        || filePath === 'api/plugins/capability-plugins/1flowbase/manifest.yaml'
        || /^scripts\/node\/docker-deploy\/.*native-react/u.test(filePath);
    },
    fast: [
      {
        id: 'native-react-page-runtime',
        command: 'pnpm',
        args: ['--dir', 'web/packages/page-runtime', 'test', '--', 'src/_tests/native-react-compiler', 'src/_tests/native-trusted-block'],
        cwd: '.',
      },
      {
        id: 'native-react-block-sdk',
        command: 'pnpm',
        args: ['--dir', 'web/packages/block-sdk', 'test', '--', 'src/_tests/native-react-contract.test.ts'],
        cwd: '.',
      },
      {
        id: 'native-react-host-composition-and-stale-artifact',
        command: 'pnpm',
        args: [
          '--dir',
          'web/app',
          'exec',
          'vitest',
          'run',
          'src/features/frontstage/_tests/native-trusted-block/native-trusted-block-runtime-factory.test.tsx',
          'src/features/frontstage/_tests/native-trusted-block/native-trusted-block-host-composition.test.tsx',
          'src/features/frontstage/_tests/runtime-cache/native-react-artifact-cache.test.ts',
        ],
        cwd: '.',
      },
      {
        id: 'native-react-host-abi',
        command: 'node',
        args: ['scripts/node/test-scripts.js', 'docker-deploy'],
        cwd: '.',
      },
    ],
    full: [
      'nightly/page-debug: complete pages, browser runtime, mobile, cache lifecycle, and visual regression',
    ],
  },
};

const GOVERNANCE_PATHS = [
  /^scripts\/node\/foundation-contracts\//u,
  /^scripts\/node\/gate-router\//u,
  /^\.github\/workflows\/foundation-contracts\.yml$/u,
  /^\.agents\/skills\/qa-evaluation\/(?:SKILL\.md|references\/governance\/foundation-contract-gates\.md)$/u,
];

function normalizeChangedFiles(changedFiles) {
  return [...new Set(changedFiles.map((filePath) => filePath.replace(/\\/gu, '/').trim()).filter(Boolean))]
    .sort((left, right) => left.localeCompare(right));
}

function validatePackInventory(overrides = {}) {
  const operations = overrides.mcpCoreOperations || MCP_CORE_OPERATIONS;
  if (operations.join(' -> ') !== MCP_CORE_OPERATIONS.join(' -> ')) {
    throw new Error(`MCP core operations must remain ${MCP_CORE_OPERATIONS.join(' -> ')}`);
  }

  for (const foundation of FOUNDATION_IDS) {
    const definition = FOUNDATION_DEFINITIONS[foundation];
    if (!definition || definition.fast.length === 0 || definition.full.length === 0) {
      throw new Error(`${foundation} must define finite fast and full evidence packs`);
    }
    const commandIds = definition.fast.map((command) => command.id);
    if (new Set(commandIds).size !== commandIds.length) {
      throw new Error(`${foundation} contains duplicate fast command ids`);
    }
  }
  return true;
}

function shouldSelectAllFoundations(changedFiles) {
  return changedFiles.some((filePath) => GOVERNANCE_PATHS.some((pattern) => pattern.test(filePath)));
}

function buildMcpFastPack(changedFiles) {
  const fast = FOUNDATION_DEFINITIONS['mcp-gateway'].fast.map((item) => ({ ...item }));
  if (changedFiles.some((filePath) => /(?:mcp_result|result_delivery|result_receipt)/iu.test(filePath))) {
    fast.push({
      id: 'mcp-result-continuation',
      command: 'cargo',
      args: ['test', '-p', 'storage-postgres', 'mcp_result_receipt_repository_tests'],
      cwd: 'api',
    });
  }
  return fast;
}

function buildFoundationPlan({ changedFiles = [], foundation = 'auto', lane = 'pr-evidence' }) {
  validatePackInventory();
  const normalizedFiles = normalizeChangedFiles(changedFiles);
  const manualSelection = foundation !== 'auto';
  const selectedFoundations = foundation === 'all'
    ? [...FOUNDATION_IDS]
    : FOUNDATION_IDS.filter((foundationId) => {
      if (manualSelection) return foundationId === foundation;
      if (shouldSelectAllFoundations(normalizedFiles)) return true;
      return normalizedFiles.some((filePath) => FOUNDATION_DEFINITIONS[foundationId].matches(filePath));
    });

  if (manualSelection && foundation !== 'all' && !FOUNDATION_IDS.includes(foundation)) {
    throw new Error(`Unknown foundation: ${foundation}`);
  }

  const packs = Object.fromEntries(FOUNDATION_IDS.map((foundationId) => {
    const definition = FOUNDATION_DEFINITIONS[foundationId];
    const matchedFiles = normalizedFiles.filter((filePath) => definition.matches(filePath));
    const reasons = manualSelection && selectedFoundations.includes(foundationId)
      ? [`manual selection: ${foundation}`]
      : matchedFiles.map((filePath) => `changed: ${filePath}`);
    if (selectedFoundations.includes(foundationId) && reasons.length === 0) {
      reasons.push('foundation governance contract changed');
    }
    return [foundationId, {
      selected: selectedFoundations.includes(foundationId),
      risk: definition.risk,
      triggerReasons: reasons,
      fast: foundationId === 'mcp-gateway' ? buildMcpFastPack(normalizedFiles) : definition.fast.map((item) => ({ ...item })),
      full: [...definition.full],
    }];
  }));

  const seamParticipants = selectedFoundations.filter((item) => (
    ['mcp-gateway', 'application-backend', 'native-react'].includes(item)
  ));
  const seams = [];
  if (seamParticipants.length > 0) {
    seams.push({
      id: 'mcp-application-native',
      participants: ['mcp-gateway', 'application-backend', 'native-react'],
      triggeredBy: seamParticipants,
      status: 'requires-participant-evidence',
    });
  }
  if (selectedFoundations.includes('ai-gateway') && normalizedFiles.some((filePath) => /agent[_-]?flow|publish/iu.test(filePath))) {
    seams.push({
      id: 'ai-gateway-published-agent-flow',
      participants: ['ai-gateway', 'application-backend'],
      triggeredBy: ['ai-gateway'],
      status: 'requires-participant-evidence',
    });
  }

  return {
    schemaVersion: '1flowbase.foundation-contract-plan/v1',
    lane,
    changedFiles: normalizedFiles,
    selectedFoundations,
    seams,
    packs,
  };
}

function buildContractReceipt({ candidateSha, plan, componentResults, eventName = '' }) {
  if (!candidateSha || !candidateSha.trim()) {
    throw new Error('candidate SHA is required');
  }
  if (!plan || !Array.isArray(plan.selectedFoundations)) {
    throw new Error('foundation contract plan is required');
  }

  const resultByFoundation = new Map(componentResults.map((result) => [result.foundation, result]));
  const foundations = plan.selectedFoundations.map((foundation) => {
    const result = resultByFoundation.get(foundation);
    if (!result) {
      return {
        foundation,
        status: 'failed',
        exitCode: 1,
        triggerReasons: plan.packs[foundation].triggerReasons,
        executedPack: plan.packs[foundation].fast.map((item) => item.id),
        warnings: [],
        warningFiles: [],
        errors: ['missing selected foundation component receipt'],
        uncovered: [],
        deferredEvidence: plan.packs[foundation].full,
      };
    }
    if (!result.candidateSha || result.candidateSha !== candidateSha.trim()) {
      return {
        foundation,
        status: 'failed',
        exitCode: 1,
        triggerReasons: plan.packs[foundation].triggerReasons,
        executedPack: result.executedPack || [],
        warnings: result.warnings || [],
        warningFiles: result.warningFiles || [],
        errors: [`candidate SHA mismatch: expected ${candidateSha.trim()}, received ${result.candidateSha || 'missing'}`],
        uncovered: result.uncovered || [],
        deferredEvidence: result.deferredEvidence || plan.packs[foundation].full,
      };
    }
    return {
      foundation,
      status: result.status,
      exitCode: result.exitCode,
      triggerReasons: plan.packs[foundation].triggerReasons,
      executedPack: result.executedPack || plan.packs[foundation].fast.map((item) => item.id),
      warnings: result.warnings || [],
      warningFiles: result.warningFiles || [],
      errors: result.errors || [],
      uncovered: result.uncovered || [],
      deferredEvidence: result.deferredEvidence || plan.packs[foundation].full,
    };
  });
  const errors = foundations.flatMap((item) => item.errors);
  const warnings = foundations.flatMap((item) => item.warnings);
  const warningFiles = [...new Set(foundations.flatMap((item) => item.warningFiles))];
  const deferredEvidence = foundations.flatMap((item) => (
    item.deferredEvidence.map((evidence) => `${item.foundation}: ${evidence}`)
  ));
  const status = foundations.some((item) => item.status !== 'passed' || item.exitCode !== 0 || item.errors.length > 0)
    ? 'failed'
    : 'passed';

  return {
    schemaVersion: '1flowbase.foundation-contract-receipt/v1',
    candidateSha: candidateSha.trim(),
    eventName,
    lane: plan.lane,
    status,
    exitCode: status === 'passed' ? 0 : 1,
    trigger: {
      changedFiles: plan.changedFiles,
      selectedFoundations: plan.selectedFoundations,
      seams: plan.seams,
    },
    foundations,
    warnings,
    warningFiles,
    errors,
    uncovered: foundations.flatMap((item) => item.uncovered),
    deferredEvidence,
  };
}

function buildQualityGateComponentReport(receipt) {
  return {
    reportType: 'ci',
    status: receipt.status,
    scope: 'foundation-contracts',
    exitCode: receipt.exitCode,
    branch: '',
    commit: receipt.candidateSha,
    warningFiles: receipt.warningFiles || [],
    coverageFiles: [],
    coverageSummaries: [],
    backendConsistencyTargets: [],
    foundationContractWarnings: receipt.warnings || [],
    errors: receipt.errors || [],
    uncovered: receipt.uncovered || [],
    deferredEvidence: receipt.deferredEvidence || [],
  };
}

function writeQualityGateComponentArtifacts(repoRoot, receipt) {
  const report = buildQualityGateComponentReport(receipt);
  const reportPath = writeJson(repoRoot, path.join(OUTPUT_ROOT, 'quality-gate-report.json'), report);
  const logPath = path.join(repoRoot, OUTPUT_ROOT, 'quality-gate.latest.log');
  fs.writeFileSync(logPath, [
    `scope=${report.scope}`,
    `status=${report.status}`,
    `exit_code=${report.exitCode}`,
    `candidate_sha=${report.commit}`,
    ...report.errors.map((error) => `error=${error}`),
    ...report.foundationContractWarnings.map((warning) => `warning=${warning}`),
  ].join('\n') + '\n', 'utf8');
  return { report, reportPath, logPath };
}

function writeJson(repoRoot, relativePath, value) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
  return absolutePath;
}

function appendGitHubPlanOutputs(plan, outputPath) {
  if (!outputPath) return;
  const lines = FOUNDATION_IDS.map((foundation) => (
    `${foundation.replace(/-/gu, '_')}=${plan.selectedFoundations.includes(foundation)}`
  ));
  lines.push(`selected_json=${JSON.stringify(plan.selectedFoundations)}`);
  lines.push(`any_selected=${plan.selectedFoundations.length > 0}`);
  fs.appendFileSync(outputPath, `${lines.join('\n')}\n`, 'utf8');
}

function runFastPack({ repoRoot, candidateSha, plan, foundation, spawnSyncImpl = spawnSync }) {
  if (!candidateSha || !candidateSha.trim()) {
    throw new Error('candidate SHA is required');
  }
  if (!plan.selectedFoundations.includes(foundation)) {
    throw new Error(`${foundation} is not selected by the plan`);
  }
  const commands = plan.packs[foundation].fast;
  const startedAt = new Date();
  const commandResults = [];

  for (const item of commands) {
    const commandStartedAt = Date.now();
    const result = spawnSyncImpl(item.command, item.args, {
      cwd: path.resolve(repoRoot, item.cwd),
      env: process.env,
      stdio: 'inherit',
    });
    const exitCode = result.error ? 1 : (result.status ?? 1);
    commandResults.push({
      id: item.id,
      command: [item.command, ...item.args].join(' '),
      exitCode,
      durationMs: Date.now() - commandStartedAt,
      error: result.error?.message || '',
    });
    if (exitCode !== 0) break;
  }

  const errors = commandResults
    .filter((item) => item.exitCode !== 0)
    .map((item) => `${item.id} exited with ${item.exitCode}${item.error ? `: ${item.error}` : ''}`);
  const componentReceipt = {
    schemaVersion: '1flowbase.foundation-contract-component/v1',
    candidateSha,
    foundation,
    status: errors.length === 0 ? 'passed' : 'failed',
    exitCode: errors.length === 0 ? 0 : 1,
    triggerReasons: plan.packs[foundation].triggerReasons,
    executedPack: commandResults.map((item) => item.id),
    commands: commandResults,
    warnings: [],
    warningFiles: [],
    errors,
    uncovered: [],
    deferredEvidence: plan.packs[foundation].full,
    startedAt: startedAt.toISOString(),
    finishedAt: new Date().toISOString(),
    durationMs: Date.now() - startedAt.getTime(),
  };
  writeJson(
    repoRoot,
    path.join(OUTPUT_ROOT, 'components', foundation, 'component-receipt.json'),
    componentReceipt,
  );
  return componentReceipt;
}

function collectComponentReceipts(componentRoot) {
  if (!fs.existsSync(componentRoot)) return [];
  const receipts = [];
  const visit = (currentPath) => {
    for (const entry of fs.readdirSync(currentPath, { withFileTypes: true })) {
      const absolutePath = path.join(currentPath, entry.name);
      if (entry.isDirectory()) visit(absolutePath);
      if (entry.isFile() && entry.name === 'component-receipt.json') {
        receipts.push(JSON.parse(fs.readFileSync(absolutePath, 'utf8')));
      }
    }
  };
  visit(componentRoot);
  return receipts;
}

function parseArgs(argv) {
  if (argv.length === 0 || argv.includes('-h') || argv.includes('--help')) return { help: true };
  const [command, ...rest] = argv;
  const values = { command, foundation: 'auto', lane: 'pr-evidence' };
  for (let index = 0; index < rest.length; index += 2) {
    const name = rest[index];
    const value = rest[index + 1];
    if (!name?.startsWith('--') || value === undefined) throw new Error(`invalid argument: ${name || 'missing'}`);
    values[name.slice(2)] = value;
  }
  return values;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/tooling.js foundation-contracts <plan|run-fast|aggregate> [options]\n'
      + '  plan --base-ref <ref> --candidate-sha <sha> [--foundation auto|all|<id>] [--event <name>]\n'
      + '  run-fast --foundation <id> --candidate-sha <sha> --plan <path>\n'
      + '  aggregate --candidate-sha <sha> --plan <path> --component-root <path> [--event <name>]\n',
  );
}

async function main(argv = [], deps = {}) {
  const options = parseArgs(argv);
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  if (options.help) {
    usage(writeStdout);
    return 0;
  }
  const repoRoot = deps.repoRoot || getRepoRoot();

  if (options.command === 'plan') {
    const { readChangedFiles } = require('../gate-router/core.js');
    const changedFiles = deps.changedFiles || readChangedFiles({
      repoRoot,
      mode: 'branch',
      baseRef: options['base-ref'] || 'origin/main',
      env: deps.env || process.env,
      spawnSyncImpl: deps.spawnSyncImpl,
    });
    const plan = buildFoundationPlan({
      changedFiles,
      foundation: options.foundation || 'auto',
      lane: options.lane || 'pr-evidence',
    });
    const planPath = writeJson(repoRoot, path.join(OUTPUT_ROOT, 'foundation-contract-plan.json'), {
      ...plan,
      candidateSha: options['candidate-sha'] || '',
      eventName: options.event || '',
    });
    appendGitHubPlanOutputs(plan, deps.githubOutputPath || process.env.GITHUB_OUTPUT);
    writeStdout(`[foundation-contracts] planned ${plan.selectedFoundations.join(', ') || 'no foundations'}: ${planPath}\n`);
    return 0;
  }

  if (options.command === 'run-fast') {
    const plan = options.plan
      ? JSON.parse(fs.readFileSync(path.resolve(repoRoot, options.plan), 'utf8'))
      : buildFoundationPlan({ changedFiles: [], foundation: options.foundation, lane: 'dev-acceptance' });
    const result = runFastPack({
      repoRoot,
      candidateSha: options['candidate-sha'],
      plan,
      foundation: options.foundation,
      spawnSyncImpl: deps.spawnSyncImpl,
    });
    return result.exitCode;
  }
  if (options.command === 'aggregate') {
    const planPath = path.resolve(repoRoot, options.plan || path.join(OUTPUT_ROOT, 'foundation-contract-plan.json'));
    const plan = JSON.parse(fs.readFileSync(planPath, 'utf8'));
    const componentRoot = path.resolve(repoRoot, options['component-root'] || path.join(OUTPUT_ROOT, 'components'));
    const receipt = buildContractReceipt({
      candidateSha: options['candidate-sha'],
      plan,
      componentResults: collectComponentReceipts(componentRoot),
      eventName: options.event || '',
    });
    const receiptPath = writeJson(repoRoot, path.join(OUTPUT_ROOT, 'foundation-contract-receipt.json'), receipt);
    writeQualityGateComponentArtifacts(repoRoot, receipt);
    writeStdout(`[foundation-contracts] ${receipt.status}: ${receiptPath}\n`);
    return receipt.exitCode;
  }
  throw new Error(`Unknown foundation-contracts command: ${options.command}`);
}

module.exports = {
  FOUNDATION_IDS,
  MCP_CORE_OPERATIONS,
  appendGitHubPlanOutputs,
  buildContractReceipt,
  buildFoundationPlan,
  buildQualityGateComponentReport,
  collectComponentReceipts,
  main,
  parseArgs,
  runFastPack,
  validatePackInventory,
  writeQualityGateComponentArtifacts,
};
