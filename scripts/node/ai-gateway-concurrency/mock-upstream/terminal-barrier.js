'use strict';

const PREFIX = '1flowbase-ws-terminal-barrier:';
function marker(key) { return `${PREFIX}${key}`; }

// Owned acceptance control: armed per request; other scenarios never wait here.
function createTerminalBarrier() {
  const states = new Map();
  return {
    arm(key) {
      if (states.has(key)) throw new Error('terminal barrier key reused');
      let release;
      let notify;
      let finish;
      states.set(key, {
        released: false, evidence: null,
        gate: new Promise((resolve) => { release = resolve; }),
        waiting: new Promise((resolve) => { notify = resolve; }),
        settled: new Promise((resolve) => { finish = resolve; }),
        finish: () => finish(),
        release: () => release(), notify: (value) => notify(value),
      });
      return marker(key);
    },
    forRequest(body, timeline) {
      const key = JSON.stringify(body).match(/1flowbase-ws-terminal-barrier:([a-zA-Z0-9-]+)/u)?.[1];
      if (!key) return null;
      const state = states.get(key);
      if (!state) throw new Error('unarmed terminal barrier');
      return async () => {
        if (state.evidence) throw new Error('terminal barrier key reached by multiple requests');
        state.evidence = { key, nonce: timeline.nonce };
        timeline.record('terminal_barrier_waiting', { barrier_key: key });
        state.notify(state.evidence);
        await state.gate;
        timeline.record('terminal_barrier_released', { barrier_key: key });
      };
    },
    completeRequest(body) {
      const key = JSON.stringify(body).match(/1flowbase-ws-terminal-barrier:([a-zA-Z0-9-]+)/u)?.[1];
      if (key) states.get(key)?.finish();
    },
    async waitSettled(key, timeoutMs = 10_000) {
      const state = states.get(key);
      if (!state) throw new Error('unknown terminal barrier');
      let timer;
      try {
        await Promise.race([state.settled, new Promise((_, reject) => {
          timer = setTimeout(() => reject(new Error('terminal barrier upstream did not settle')), timeoutMs);
        })]);
      } finally { clearTimeout(timer); }
    },
    async wait(key, timeoutMs = 10_000) {
      const state = states.get(key);
      if (!state) throw new Error('unknown terminal barrier');
      let timer;
      try {
        return await Promise.race([state.waiting, new Promise((_, reject) => {
          timer = setTimeout(() => reject(new Error('terminal barrier did not reach first delta')), timeoutMs);
        })]);
      } finally { clearTimeout(timer); }
    },
    release(key) {
      const state = states.get(key);
      if (!state || state.released) return false;
      state.released = true;
      state.release();
      return true;
    },
    close() { for (const state of states.values()) state.release(); states.clear(); },
  };
}

module.exports = { createTerminalBarrier, marker };
