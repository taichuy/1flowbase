'use strict';
const { EventEmitter } = require('node:events');
const readline = require('node:readline');
class Rpc extends EventEmitter {
  constructor(child, record) {
    super(); this.child = child; this.record = record; this.next = 1; this.pending = new Map(); this.history = [];
    readline.createInterface({ input: child.stdout }).on('line', (line) => {
      let message;
      try { message = JSON.parse(line); } catch { this.fail(new Error('Non-JSON app-server stdout')); return; }
      this.record({ kind: 'rpc/receive', ...message }); this.history.push(message);
      if (message.id !== undefined && this.pending.has(message.id) && !message.method) {
        const waiter = this.pending.get(message.id); this.pending.delete(message.id); clearTimeout(waiter.timer);
        if (message.error) waiter.reject(new Error(`RPC ${waiter.method}: ${JSON.stringify(message.error)}`));
        else waiter.resolve(message.result);
      } else if (message.id !== undefined && message.method) {
        this.send({ id: message.id, error: { code: -32601, message: 'No approvals or dynamic tools accepted by this read-only fixture' } });
      }
      this.emit('message', message);
    });
    child.on('exit', (code, signal) => { this.record({ kind: 'client/exit', code, signal }); this.fail(new Error(`app-server exited ${code}/${signal}`)); });
    child.on('error', (error) => this.fail(error));
  }
  fail(error) { this.failure = error; for (const p of this.pending.values()) { clearTimeout(p.timer); p.reject(error); } this.pending.clear(); this.emit('failure', error); }
  send(message) { this.record({ kind: message.method ? 'rpc/request' : 'rpc/reply', ...message }); this.child.stdin.write(`${JSON.stringify(message)}\n`); }
  request(method, params, timeoutMs = 120000) {
    if (this.failure) return Promise.reject(this.failure);
    const id = this.next++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`RPC timeout ${method}`)); }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer, method }); this.send({ id, method, params });
    });
  }
  wait(predicate, timeoutMs = 900000, since = 0, onMatch) {
    const seen = this.history.slice(since).find(predicate);
    if (seen) { try { if (onMatch) onMatch(seen); return Promise.resolve(seen); } catch (error) { return Promise.reject(error); } }
    if (this.failure) return Promise.reject(this.failure);
    return new Promise((resolve, reject) => {
      const clean = () => { clearTimeout(timer); this.off('message', onMessage); this.off('failure', onFailure); };
      const onMessage = (message) => { if (predicate(message)) { clean(); try { if (onMatch) onMatch(message); resolve(message); } catch (error) { reject(error); } } };
      const onFailure = (error) => { clean(); reject(error); };
      const timer = setTimeout(() => { clean(); reject(new Error('Protocol event deadline exceeded')); }, timeoutMs);
      this.on('message', onMessage); this.on('failure', onFailure);
    });
  }
}
module.exports = { Rpc };
