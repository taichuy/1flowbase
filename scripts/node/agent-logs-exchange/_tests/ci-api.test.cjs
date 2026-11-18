const test = require('node:test');
const assert = require('node:assert/strict');
const { buildApiTestCommands } = require('../ci-api.cjs');
const { requireExecutedCargoTests } = require('../../verify/cargo-test-results.js');

// Deliberately put a same-filter bystander in the wrong binary. Command selection,
// rather than a positive total from another suite, must choose each original case.
const inventory = {
  api_server: [
    { id: 'private-exchange', name: 'host::exchange_host_services' },
    { id: 'private-codec', name: 'agent_logs_protocol::decode' },
    { id: 'wrong-route', name: 'application_api_routes::otlp_logs::bystander' },
  ],
  extension_graph_contract: [
    { id: 'public-exchange', name: 'input_assembly_tests::exchange_host_services_and_file_read' },
  ],
  api_application: [
    { id: 'otlp-http', name: '_tests::application::application_api_routes::otlp_logs::accept' },
    { id: 'exchange-http', name: '_tests::application::logs_routes::exchange::import' },
    { id: 'native-ingress', name: '_tests::application::application_api_routes::log_reception::append' },
    { id: 'native-export', name: 'application_runtime_routes_logs_export_json_trace_dump_preserves_detail_and_error_payload' },
    { id: 'native-archive', name: 'application_runtime_routes_logs_archive_returns_v1_manifest_and_restore_facts' },
  ],
  api_documentation: [
    { id: 'otlp-openapi', name: 'canonical_otlp_trace_ingest_advertises_application_bearer_and_standard_wire_formats' },
  ],
};
function select({ args }, available = inventory) {
  const targetIndex = args.indexOf('--test');
  const binary = targetIndex < 0 ? 'api_server' : args[targetIndex + 1];
  const filter = args[args.indexOf('--') - 1];
  return (available[binary] || []).filter(({ name }) => name.includes(filter));
}
function output(cases) {
  return `running ${cases.length} tests\n${cases.map(({ name }) => `test ${name} ... ok\n`).join('')}test result: ok. ${cases.length} passed; 0 failed; 0 ignored;\n`;
}

test('agent-log command routing preserves both exchange halves and all migrated contracts exactly once', () => {
  const relevant = new Set(['codec', 'public-route', 'exchange-host-api', 'exchange-host-graph', 'console-route', 'openapi', 'native-reception', 'native-export', 'native-archive']);
  const commands = buildApiTestCommands(2).filter(({ label }) => relevant.has(label));
  const actual = commands.flatMap((command) => {
    const cases = select(command);
    requireExecutedCargoTests(output(cases));
    return cases.map(({ id }) => id);
  });
  assert.deepEqual(actual.sort(), [
    'private-codec', 'private-exchange', 'public-exchange', 'otlp-http', 'exchange-http',
    'native-ingress', 'native-export', 'native-archive', 'otlp-openapi',
  ].sort());
  for (const { args } of commands) {
    assert.equal(args[args.indexOf('--features') + 1], 'tikv-jemallocator/stats');
    assert.ok(args.includes('profile.dev.package.api-server.debug=0'));
    assert.ok(args.includes('profile.test.package.api-server.debug=0'));
    assert.deepEqual(args.slice(args.indexOf('--')), ['--', '--test-threads', '2']);
  }
});

test('one empty exchange half is rejected even when its neighboring lib half passes', () => {
  const commands = buildApiTestCommands(1);
  const lib = commands.find(({ label }) => label === 'exchange-host-api');
  const native = commands.find(({ label }) => label === 'exchange-host-graph');
  requireExecutedCargoTests(output(select(lib)));
  assert.throws(() => requireExecutedCargoTests(output(select(native, {
    ...inventory, extension_graph_contract: [],
  }))), /no executed passing tests/u);
});
