const ACTIONS = new Set(['start', 'ensure', 'stop', 'status', 'restart']);
const SCOPES = new Set(['all', 'frontend', 'backend']);

function usage() {
  process.stdout.write(`Usage: node scripts/node/dev-up.js [options] [start|ensure|stop|status|restart]

Default action: start

Options:
  --frontend-only  Manage the frontend process only
  --backend-only   Manage backend processes only (api-server + plugin-runner)
  --skip-docker    Skip Docker middleware management
  -h, --help       Show this help

Examples:
  node scripts/node/dev-up.js
  node scripts/node/dev-up.js --skip-docker
  node scripts/node/dev-up.js restart --frontend-only
  node scripts/node/dev-up.js restart --backend-only
  node scripts/node/dev-up.js status
`);
}

function log(message) {
  process.stdout.write(`[1flowbase-dev-up] ${message}\n`);
}

function parseCliArgs(argv) {
  let action = 'start';
  let actionSpecified = false;
  let scope = 'all';
  let skipDocker = false;
  let help = false;

  for (const arg of argv) {
    if (arg === '-h' || arg === '--help') {
      help = true;
      continue;
    }

    if (arg === '--frontend-only') {
      if (scope !== 'all') {
        throw new Error('Cannot specify --frontend-only and --backend-only together');
      }
      scope = 'frontend';
      continue;
    }

    if (arg === '--backend-only') {
      if (scope !== 'all') {
        throw new Error('Cannot specify --frontend-only and --backend-only together');
      }
      scope = 'backend';
      continue;
    }

    if (arg === '--skip-docker') {
      skipDocker = true;
      continue;
    }

    if (arg.startsWith('-')) {
      throw new Error(`Unknown option: ${arg}`);
    }

    if (actionSpecified) {
      throw new Error(`Only one action may be specified; unexpected argument: ${arg}`);
    }

    if (!ACTIONS.has(arg)) {
      throw new Error(`Unknown action: ${arg}`);
    }

    action = arg;
    actionSpecified = true;
  }

  if (!SCOPES.has(scope)) {
    throw new Error(`Unknown scope: ${scope}`);
  }

  return {
    action,
    scope,
    skipDocker,
    help,
  };
}

function shouldManageDocker(options) {
  return !options.skipDocker && options.scope === 'all';
}

function selectServiceKeys(scope) {
  switch (scope) {
    case 'frontend':
      return ['web'];
    case 'backend':
      return ['api-server', 'plugin-runner'];
    default:
      return ['web', 'api-server', 'plugin-runner'];
  }
}

module.exports = {
  log,
  parseCliArgs,
  selectServiceKeys,
  shouldManageDocker,
  usage,
};
