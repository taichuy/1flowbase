'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const { CLIENT_NAMES } = require('./lock');

function verifyRuntimeLock(lock, runtimeRoot) {
  const nodeMajor = Number.parseInt(process.versions.node.split('.')[0], 10);
  if (nodeMajor !== lock.node_major) throw new Error(`client runtime requires Node ${lock.node_major}, received ${nodeMajor}`);
  const lockPath = path.join(path.resolve(runtimeRoot), 'package-lock.json');
  let runtimeLock;
  try {
    runtimeLock = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
  } catch (error) {
    throw new Error(`client runtime package lock is unavailable: ${error.message}`);
  }
  for (const [key, spec] of Object.entries(lock.packages)) {
    const installed = runtimeLock.packages?.[`node_modules/${spec.name}`];
    if (installed?.version !== spec.version || installed?.integrity !== spec.integrity) {
      throw new Error(`client runtime package ${key} does not match committed version and integrity`);
    }
  }
  return lockPath;
}

function sha256File(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function resolveRuntimePath(runtimeRoot, relativePath, label) {
  const root = path.resolve(runtimeRoot);
  const resolved = path.resolve(root, relativePath);
  if (resolved !== root && !resolved.startsWith(`${root}${path.sep}`)) throw new Error(`${label} escapes runtime root`);
  let real;
  try {
    real = fs.realpathSync(resolved);
  } catch (error) {
    throw new Error(`${label} is unavailable: ${error.message}`);
  }
  if (real !== root && !real.startsWith(`${root}${path.sep}`)) throw new Error(`${label} resolves outside runtime root`);
  if (!fs.statSync(real).isFile()) throw new Error(`${label} is not a file`);
  return real;
}

function resolveClients(lock, runtimeRoot) {
  verifyRuntimeLock(lock, runtimeRoot);
  return Object.fromEntries(CLIENT_NAMES.map((name) => {
    const client = lock.clients[name];
    const executable = resolveRuntimePath(runtimeRoot, client.executable, `${name} executable`);
    const adapterExecutable = resolveRuntimePath(runtimeRoot, client.adapter_executable, `${name} adapter`);
    return [name, {
      name,
      gateway_protocol: client.gateway_protocol,
      executable,
      executable_sha256: sha256File(executable),
      adapter_executable: adapterExecutable,
      adapter_sha256: sha256File(adapterExecutable),
      adapter_args: [...(client.adapter_args ?? [])],
      binding_env: client.binding_env,
      client_package: lock.packages[client.package],
      platform_package: lock.packages[client.platform_package],
      adapter_package: client.adapter_package === null ? null : lock.packages[client.adapter_package],
    }];
  }));
}

module.exports = {
  resolveClients,
  resolveRuntimePath,
  sha256File,
  verifyRuntimeLock,
};
