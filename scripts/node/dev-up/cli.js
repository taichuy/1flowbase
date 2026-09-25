const ACTIONS = new Set(['start', 'ensure', 'stop', 'status', 'restart']);
const SCOPES = new Set(['all', 'frontend', 'backend']);

function usage() {
  process.stdout.write(`Usage: node scripts/node/dev-up.js [options] [start|ensure|stop|status|restart]

Default action: restart (rebuild and restart services from current sources)
The default action reuses a running PostgreSQL or starts it when absent.

Use start or ensure explicitly to reuse healthy services.

Options:
  --frontend-only  Manage the frontend process only
  --backend-only   Manage the api-server backend process only
  --skip-docker    Skip Docker middleware management
  -h, --help       Show this help

Examples:
  node scripts/node/dev-up.js
  node scripts/node/dev-up.js start
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
  let action = 'restart';
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
    actionSpecified,
    scope,
    skipDocker,
    help,
  };
}

function getMiddlewareAction(options) {
  return options.action === 'restart' && !options.actionSpecified ? 'start' : options.action;
}

function shouldManageDocker(options) {
  return !options.skipDocker && options.scope === 'all';
}

function selectServiceKeys(scope) {
  switch (scope) {
    case 'frontend':
      return ['web'];
    case 'backend':
      return ['api-server'];
    default:
      return ['web', 'api-server'];
  }
}

module.exports = {
  getMiddlewareAction,
  log,
  parseCliArgs,
  selectServiceKeys,
  shouldManageDocker,
  usage,
};
