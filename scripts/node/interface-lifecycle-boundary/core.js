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
  const contributions = read(repoRoot, 'api/apps/api-server/src/extension_bus/interface_contributions.rs');
  const publicAuth = read(repoRoot, 'api/apps/api-server/src/routes/identity/auth.rs');
  const publicSignIn = read(repoRoot, 'api/apps/api-server/src/routes/identity/sign_in_interface.rs');
  const native = read(repoRoot, 'api/apps/api-server/src/routes/application_public_api/native.rs');
  const compatibility = read(repoRoot, 'api/apps/api-server/src/routes/application_public_api/compatibility_interface.rs');
  const mcp = read(repoRoot, 'api/apps/api-server/src/routes/mcp_protocol.rs');
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
  if (contributions.includes('type CompileInterfaceRegistry') || contributions.includes('Weak<crate::app_state::ApiState>')) {
    violations.push('Interface contribution collector retains a global-state compile callback');
  }
  if (contributions.includes('collector.compile(Arc::downgrade')) {
    violations.push('Interface contribution publication forwards global state into the collector');
  }
  for (const [name, source] of [
    ['public auth', publicAuth],
    ['public sign-in', publicSignIn],
    ['native', native],
    ['compatibility', compatibility],
    ['workflow extension', workflowExtension],
    ['MCP', mcp],
  ]) {
    if (/struct\s+\w*Adapter\s*\{[^}]*\b(?:Weak\s*<\s*)?(?:Arc\s*<\s*)?ApiState\b/su.test(source)) {
      violations.push(`${name} Interface adapter retains the global ApiState container`);
    }
    const stateTypes = ['ApiState'];
    for (const match of source.matchAll(/type\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?:crate::app_state::)?ApiState\s*;/gu)) {
      stateTypes.push(match[1]);
    }
    for (const stateType of stateTypes) {
      const escaped = stateType.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
      if (new RegExp(`impl\\s+[A-Za-z_][A-Za-z0-9_]*Port\\s+for\\s+${escaped}\\b`, 'u').test(source)) {
        violations.push(`${name} erases the global ApiState behind a narrow Port trait`);
      }
    }
  }
  if (/compile_[A-Za-z0-9_]*registry\s*\(\s*(?:Arc::clone\(state\)|state\.clone\(\))/u.test(contributions)
      || /(?:Arc::clone\(state\)|state\.clone\(\))\s+as\s+Arc<dyn\s+\w*Port>/u.test(contributions)) {
    violations.push('Composition Root casts the global ApiState directly to a family Port');
  }
  return violations;
}

module.exports = { LEGACY_COMPATIBILITY_SYMBOLS, inspectInterfaceLifecycleBoundary };
