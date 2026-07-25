'use strict';

const readline = require('node:readline');

const { redact } = require('./redact');

class AcpRpcConnection {
  constructor(child, options = {}) {
    this.child = child;
    this.timeoutMs = options.timeoutMs ?? 120000;
    this.onNotification = options.onNotification ?? (() => {});
    this.onRequest = options.onRequest ?? (() => null);
    this.record = options.record ?? (() => {});
    this.secrets = options.secrets ?? [];
    this.nextId = 1;
    this.pending = new Map();
    this.stderr = '';
    this.closed = false;
    this.fatalError = null;
    this.lines = readline.createInterface({ input: child.stdout, crlfDelay: Infinity });
    this.lines.on('line', (line) => this.handleLine(line));
    child.stderr.on('data', (chunk) => {
      this.stderr = `${this.stderr}${chunk}`.slice(-8192);
    });
    child.once('error', (error) => this.failPending(error));
    child.once('exit', (code, signal) => {
      if (!this.closed) this.failPending(new Error(`ACP process exited before close: code=${code} signal=${signal}`));
    });
  }

  send(message) {
    if (this.closed || !this.child.stdin.writable) throw new Error('ACP connection is closed');
    if (this.fatalError) throw this.fatalError;
    this.record('acp_send', redact(message, this.secrets));
    this.child.stdin.write(`${JSON.stringify(message)}\n`);
  }

  request(method, params, timeoutMs = this.timeoutMs) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`ACP request timed out: ${method}`));
      }, timeoutMs);
      this.pending.set(id, { method, resolve, reject, timer });
      try {
        this.send({ jsonrpc: '2.0', id, method, params });
      } catch (error) {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(error);
      }
    });
  }

  notify(method, params) {
    this.send({ jsonrpc: '2.0', method, params });
  }

  respond(id, result) {
    this.send({ jsonrpc: '2.0', id, result });
  }

  respondError(id, code, message) {
    this.send({ jsonrpc: '2.0', id, error: { code, message } });
  }

  async handleLine(line) {
    if (!line.trim()) return;
    let message;
    try {
      message = JSON.parse(line);
    } catch (error) {
      this.failPending(new Error(`ACP emitted invalid JSON: ${error.message}`));
      return;
    }
    this.record('acp_receive', redact(message, this.secrets));
    if (message.id !== undefined && message.method === undefined) {
      const pending = this.pending.get(message.id);
      if (!pending) {
        const error = new Error(`ACP emitted orphan response id: ${message.id}`);
        this.fatalError = error;
        this.failPending(error);
        return;
      }
      clearTimeout(pending.timer);
      this.pending.delete(message.id);
      if (message.error) pending.reject(new Error(`ACP ${pending.method} failed: ${message.error.message ?? 'unknown error'}`));
      else pending.resolve(message.result ?? {});
      return;
    }
    if (typeof message.method !== 'string') return;
    if (message.id === undefined) {
      await this.onNotification(message.method, message.params ?? {});
      return;
    }
    try {
      const result = await this.onRequest(message.method, message.params ?? {});
      if (result === null || result === undefined) this.respondError(message.id, -32601, `unsupported ACP client method: ${message.method}`);
      else this.respond(message.id, result);
    } catch (error) {
      this.respondError(message.id, -32603, error.message);
    }
  }

  failPending(error) {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.pending.clear();
  }

  async close() {
    if (this.closed) return;
    this.closed = true;
    this.lines.close();
    this.child.stdin.end();
    if (this.child.exitCode === null && this.child.signalCode === null) this.child.kill('SIGTERM');
    await Promise.race([
      new Promise((resolve) => this.child.once('exit', resolve)),
      new Promise((resolve) => setTimeout(resolve, 2000)),
    ]);
    if (this.child.exitCode === null && this.child.signalCode === null) this.child.kill('SIGKILL');
    this.failPending(new Error('ACP connection closed'));
  }
}

module.exports = { AcpRpcConnection };
