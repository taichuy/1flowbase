const fs = require('node:fs');
const path = require('node:path');

const LEGACY_COMPATIBILITY_SYMBOLS = Object.freeze([
  'PreparedCompatibleTurn',
  'start_compatible_turn_stream',
  'start_openai_run_stream',
  'start_openai_response_stream',
  'start_anthropic_run_stream',
  'authenticate_openai_response_credential',
  'execute_openai_tool_resume',
  'execute_anthropic_tool_resume',
]);

function read(repoRoot, relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

function inspectInterfaceLifecycleBoundary(repoRoot) {
  const stream = read(repoRoot, 'api/apps/api-server/src/routes/application_public_api/compat_sse.rs');
  const openai = read(repoRoot, 'api/apps/api-server/src/routes/application_public_api/openai.rs');
  const anthropic = read(repoRoot, 'api/apps/api-server/src/routes/application_public_api/anthropic.rs');
  const workflowExtension = read(repoRoot, 'api/apps/api-server/src/routes/application_public_api/ex.rs');
  const protocolSources = `${stream}\n${openai}\n${anthropic}`;
  const violations = [];

  for (const symbol of LEGACY_COMPATIBILITY_SYMBOLS) {
    if (protocolSources.includes(symbol)) {
      violations.push(`legacy compatibility execution owner remains: ${symbol}`);
    }
  }
  if (stream.includes('public_mcp_runtime_invoker(&state')) {
    violations.push('compatibility stream reauthenticates from a raw bearer token');
  }
  for (const [name, source] of [['OpenAI', openai], ['Anthropic', anthropic]]) {
    if (!source.includes('compatibility_interface::invoke_blocking')) {
      violations.push(`${name} blocking route bypasses the compiled invocation plan`);
    }
    if (!source.includes('compatibility_interface::invoke_stream')) {
      violations.push(`${name} stream route bypasses the compiled invocation plan`);
    }
  }
  if ((workflowExtension.match(/\.authenticate\(/gu) || []).length !== 1) {
    violations.push('/api/ex must have exactly one frozen Authentication owner');
  }
  if (workflowExtension.includes('require_session(&state') || workflowExtension.includes('require_csrf(&headers')) {
    violations.push('/api/ex retains a direct authentication or CSRF bypass');
  }
  return violations;
}

module.exports = { LEGACY_COMPATIBILITY_SYMBOLS, inspectInterfaceLifecycleBoundary };
