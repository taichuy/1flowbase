'use strict';

const { SCENARIO, TRANSPORT } = require('../contracts');

const CHARACTERIZE_CONCURRENCY = Object.freeze([1, 4, 16, 32]);
const TRANSPORTS = Object.freeze(Object.values(TRANSPORT));
const TOPOLOGY = Object.freeze({ SAME_POOL: 'same-pool', MULTI_POOL: 'multi-pool' });

function row(transport, scenario, concurrency, topology = TOPOLOGY.SAME_POOL) {
  return Object.freeze({ transport, scenario, concurrency, topology });
}

const CHARACTERIZE_PLAN = Object.freeze([
  ...CHARACTERIZE_CONCURRENCY.flatMap((concurrency) => TRANSPORTS.map(
    (transport) => row(transport, SCENARIO.NORMAL, concurrency)
  )),
  ...TRANSPORTS.map((transport) => row(transport, SCENARIO.SLOW, 4)),
  ...[SCENARIO.CANCEL_OBSERVATION, SCENARIO.HTTP_500, SCENARIO.STREAM_INTERRUPTION]
    .flatMap((scenario) => TRANSPORTS.map((transport) => row(transport, scenario, 1))),
  ...CHARACTERIZE_CONCURRENCY.map((concurrency) => row(
    TRANSPORT.ANTHROPIC_SSE, SCENARIO.NORMAL, concurrency, TOPOLOGY.MULTI_POOL
  )),
  row(TRANSPORT.ANTHROPIC_SSE, SCENARIO.SLOW, 4, TOPOLOGY.MULTI_POOL),
]);

module.exports = { CHARACTERIZE_CONCURRENCY, CHARACTERIZE_PLAN, TOPOLOGY };
