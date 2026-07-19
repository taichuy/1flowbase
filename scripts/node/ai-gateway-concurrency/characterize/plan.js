'use strict';

const { SCENARIO, TRANSPORT } = require('../contracts');

const CHARACTERIZE_CONCURRENCY = Object.freeze([1, 4, 16, 32]);
const TRANSPORTS = Object.freeze(Object.values(TRANSPORT));

const CHARACTERIZE_PLAN = Object.freeze([
  ...CHARACTERIZE_CONCURRENCY.flatMap((concurrency) => TRANSPORTS.map((transport) => Object.freeze({
    transport,
    scenario: SCENARIO.NORMAL,
    concurrency,
  }))),
  ...TRANSPORTS.map((transport) => Object.freeze({ transport, scenario: SCENARIO.SLOW, concurrency: 4 })),
  ...[SCENARIO.CANCEL_OBSERVATION, SCENARIO.HTTP_500, SCENARIO.STREAM_INTERRUPTION]
    .flatMap((scenario) => TRANSPORTS.map((transport) => Object.freeze({ transport, scenario, concurrency: 1 }))),
]);

module.exports = { CHARACTERIZE_CONCURRENCY, CHARACTERIZE_PLAN };
