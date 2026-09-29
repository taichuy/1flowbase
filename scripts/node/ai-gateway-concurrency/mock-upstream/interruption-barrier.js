'use strict';

// Only an actual receiver's nonce-correlated acknowledgement permits destruction.
function createInterruptionBarrier() {
  const states = new Map();
  return {
    arm(timeline) {
      if (states.has(timeline.nonce)) throw new Error('interruption nonce reused');
      let resolve;
      const gate = new Promise((done) => { resolve = done; });
      const state = { waiting: false, acknowledged: false, resolve, timeline };
      states.set(timeline.nonce, state);
      return {
        async wait() {
          state.waiting = true;
          timeline.record('interruption_barrier_waiting');
          await gate;
          return state.acknowledged;
        },
        abandon() { states.delete(timeline.nonce); resolve(); },
      };
    },
    release(nonce) {
      const state = states.get(nonce);
      if (!state || !state.waiting || state.acknowledged) return false;
      state.acknowledged = true;
      state.timeline.record('interruption_delta_acknowledged');
      state.resolve();
      return true;
    },
    close() { for (const state of states.values()) state.resolve(); states.clear(); },
  };
}

module.exports = { createInterruptionBarrier };
