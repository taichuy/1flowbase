const fs = require('node:fs');
const path = require('node:path');
const { productionRustSources, bootRustSources } = require('../testing/contracts/rust-sources.js');

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
  return productionRustSources(path.join(repoRoot, relativePath));
}

function rustSourcesUnder(repoRoot, relativeRoot) {
  const root = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(root)) return [];
  const result = [];
  const visit = (directory) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const target = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        if (entry.name !== '_tests' && entry.name !== 'tests') visit(target);
      } else if (entry.isFile() && entry.name.endsWith('.rs') && entry.name !== 'tests.rs') {
        result.push([path.relative(repoRoot, target), fs.readFileSync(target, 'utf8')]);
      }
    }
  };
  visit(root);
  return result;
}

// This narrow source gate protects the final unwrapping boundary, where Axum must
// regain ownership for middleware and serving. Inner merges are type checked.
function rustFunctionBody(source, name) {
  const declaration = new RegExp(`\\bfn\\s+${name}\\s*\\(`, 'u').exec(source);
  if (!declaration) return '';
  const start = source.indexOf('{', declaration.index);
  let depth = 1;
  let end = start + 1;
  while (end < source.length && depth) {
    if (source[end] === '{') depth += 1;
    if (source[end] === '}') depth -= 1;
    end += 1;
  }
  return source.slice(start + 1, end - 1);
}

function inspectExternalRouteAssemblyBoundary(apiServer) {
  const violations = [];
  const assembly = rustFunctionBody(apiServer, 'console_router_with_assembly');
  if (!assembly.includes('ExternalRouteAssembly::new()')
      || !assembly.includes('router.contributions()')
      || !/router\s*\.into_router\(\)/u.test(assembly)) {
    violations.push('production router must retain the executable external route inventory');
  }
  const terminal = assembly.slice(assembly.search(/router\s*\.into_router\(\)/u));
  if (/\bRouter\s*::\s*new\s*\(/u.test(assembly)
      || /\.(?:route|route_service|nest|nest_service|merge|fallback|fallback_service)\s*\(/u.test(terminal)) {
    violations.push('production router contains a bare external mount after or outside controlled assembly');
  }
  for (const name of ['app_with_state', 'app_with_state_and_config_and_console_route_assembly']) {
    const body = rustFunctionBody(apiServer, name);
    if (/\.(?:route|route_service|nest|nest_service|merge|fallback|fallback_service)\s*\(/u.test(body)) {
      violations.push(`${name} contains a bare external mount outside controlled assembly`);
    }
  }
  return violations;
}

function inspectAuthenticationInvocationBoundary(relativePath, source) {
  const violations = [];
  if (/\.authenticate\s*(?:::<[^>]+>)?\s*\(/u.test(source)) {
    violations.push(`${relativePath} bypasses the authentication invocation owner`);
  }
  if (source.includes('.authenticate_invocation') && !source.includes('.into_envelope(')) {
    violations.push(`${relativePath} discards authenticated attempt lineage`);
  }
  return violations;
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
  const apiServer = bootRustSources(repoRoot);
  const endpointCatalog = read(repoRoot, 'api/apps/api-server/src/console_contracts/external_endpoint_catalog.rs');
  const protocolSources = `${stream}\n${openai}\n${anthropic}`;
  const violations = inspectExternalRouteAssemblyBoundary(apiServer);

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
    if (!source.includes('compatibility_interface::invoke_client_stream_with_principal')) {
      violations.push(`${name} stream route bypasses the compiled invocation plan`);
    }
  }
  const clientStream = rustFunctionBody(compatibility, 'invoke_client_stream_with_principal');
  const typedStreamWrapper = rustFunctionBody(compatibility, 'invoke_typed_stream_with_principal');
  // The reservation release wrapper awaits the same principal-bearing invocation.
  // Only follow this explicit edge; an unrelated inner function is not evidence.
  const typedStream = /invoke_typed_stream_with_principal_inner\(state,\s*binding_id,\s*principal,\s*input\)\.await/u.test(typedStreamWrapper)
    ? rustFunctionBody(compatibility, 'invoke_typed_stream_with_principal_inner')
    : typedStreamWrapper;
  if (!clientStream.includes('invoke_typed_stream_with_principal(')
      || !clientStream.includes('project_stream_invocation(')
      || !typedStream.includes('principal.into_envelope(')
      || !typedStream.includes('interface_runtime::InterfaceInvocationKernel::new(')
      || !typedStream.includes('.invoke_server_stream_with_dispatch_target::<')) {
    violations.push('compatibility client stream must preserve principal, compiled invocation and projection ownership');
  }
  if ((workflowExtension.match(/\.authenticate_invocation(?=\s*::|\s*\()/gu) || []).length !== 1) {
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
  // Composition may extract typed services from state while compiling a registry.
  // Reject actual Port erasure and retained adapter state, not the compiler input.
  if (/(?:Arc::clone\(state\)|state\.clone\(\))\s+as\s+Arc<dyn\s+\w*Port>/u.test(contributions)) {
    violations.push('Composition Root casts the global ApiState directly to a family Port');
  }
  for (const [relativePath, source] of rustSourcesUnder(repoRoot, 'api/apps/api-server/src/routes')) {
    violations.push(...inspectAuthenticationInvocationBoundary(relativePath, source));
    if (/struct\s+\w*Adapter\s*\{[^}]*\b(?:Weak\s*<\s*)?(?:Arc\s*<\s*)?ApiState\b/su.test(source)) {
      violations.push(`${relativePath} Interface adapter retains the global ApiState container`);
    }
    const stateTypes = ['ApiState'];
    for (const match of source.matchAll(/type\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?:crate::app_state::)?ApiState\s*;/gu)) {
      stateTypes.push(match[1]);
    }
    for (const stateType of stateTypes) {
      const escaped = stateType.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
      if (new RegExp(`impl\\s+[A-Za-z_][A-Za-z0-9_]*Port\\s+for\\s+${escaped}\\b`, 'u').test(source)) {
        violations.push(`${relativePath} erases the global ApiState behind a narrow Port trait`);
      }
    }
    // This signal-only transport is a declared Protocol Control, not a business
    // invocation. The exact catalog declaration must remain present; a pathname
    // or comment alone never exempts direct authentication in ordinary routes.
    const notificationProtocolControl = relativePath === 'api/apps/api-server/src/routes/notifications/events.rs'
      && /Self::NotificationEvents\s*=>\s*ExternalEndpointContribution::protocol_control_http\(\s*"notification-refresh-signal",\s*"GET",\s*"\/api\/console\/notifications\/events",\s*\)/u.test(endpointCatalog);
    if ((source.includes('require_session(&state') || source.includes('require_csrf(&headers'))
      && relativePath !== 'api/apps/api-server/src/routes/assistant/websocket.rs'
      && relativePath !== 'api/apps/api-server/src/routes/settings/system_backups/mod.rs'
      && !notificationProtocolControl) {
      violations.push(`${relativePath} retains a direct route authentication owner`);
    }
  }
  if (!apiServer.includes('publish_external_endpoint_catalog(')
      || !apiServer.includes('contribute_openapi_document(')
      || !endpointCatalog.includes('compile_complete(')
      || !endpointCatalog.includes('UnclassifiedRows')) {
    violations.push('Composition Root does not fail closed on the complete external endpoint catalog');
  }
  return violations;
}

module.exports = { LEGACY_COMPATIBILITY_SYMBOLS, inspectInterfaceLifecycleBoundary, inspectExternalRouteAssemblyBoundary, inspectAuthenticationInvocationBoundary };
