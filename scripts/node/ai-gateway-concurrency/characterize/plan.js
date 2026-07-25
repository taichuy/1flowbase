'use strict';

const { SCENARIO, TRANSPORT } = require('../contracts');

const GATE_ROLE = Object.freeze({
  BLOCKING: 'blocking-correctness',
  ADVISORY: 'non-blocking-performance',
});
const CORRECTNESS_CONCURRENCY = Object.freeze([1, 4]);
const PERFORMANCE_CONCURRENCY = Object.freeze([16, 32]);
const TRANSPORTS = Object.freeze(Object.values(TRANSPORT));
const TOPOLOGY = Object.freeze({ SAME_POOL: 'same-pool', MULTI_POOL: 'multi-pool' });

function row(transport, scenario, concurrency, topology = TOPOLOGY.SAME_POOL, gateRole = GATE_ROLE.BLOCKING) {
  return Object.freeze({ transport, scenario, concurrency, topology, gateRole });
}

const CHARACTERIZE_PLAN = Object.freeze([
  ...Object.values(SCENARIO).flatMap((scenario) => TRANSPORTS.map(
    (transport) => row(transport, scenario, 1)
  )),
  ...TRANSPORTS.map((transport) => row(transport, SCENARIO.NORMAL, 4)),
  row(TRANSPORT.ANTHROPIC_SSE, SCENARIO.NORMAL, 2, TOPOLOGY.MULTI_POOL),
  ...PERFORMANCE_CONCURRENCY.flatMap((concurrency) => TRANSPORTS.map(
    (transport) => row(transport, SCENARIO.NORMAL, concurrency, TOPOLOGY.SAME_POOL, GATE_ROLE.ADVISORY)
  )),
  ...PERFORMANCE_CONCURRENCY.map((concurrency) => row(
    TRANSPORT.ANTHROPIC_SSE, SCENARIO.NORMAL, concurrency, TOPOLOGY.MULTI_POOL, GATE_ROLE.ADVISORY
  )),
  row(TRANSPORT.ANTHROPIC_SSE, SCENARIO.SLOW, 4, TOPOLOGY.MULTI_POOL, GATE_ROLE.ADVISORY),
]);

module.exports = {
  CHARACTERIZE_PLAN,
  CORRECTNESS_CONCURRENCY,
  GATE_ROLE,
  PERFORMANCE_CONCURRENCY,
  TOPOLOGY,
};
