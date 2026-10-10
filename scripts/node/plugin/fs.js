const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

function log(message, options = {}) {
  if (options.silent) {
    return;
  }

  process.stdout.write(`[1flowbase-plugin] ${message}\n`);
}

function sanitizeCode(value) {
  return String(value || '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .replace(/_+/g, '_');
}

function assertNonEmptyCode(value, label) {
  if (!value) {
    throw new Error(`${label} 不能为空`);
  }

  return value;
}

function getPluginName(pluginPath) {
  return path.basename(path.resolve(pluginPath));
}

function ensureTargetDirForInit(pluginPath) {
  if (!fs.existsSync(pluginPath)) {
    fs.mkdirSync(pluginPath, { recursive: true });
    return;
  }

  const entries = fs.readdirSync(pluginPath);
  if (entries.length > 0) {
    throw new Error(`目标目录非空，拒绝覆盖：${pluginPath}`);
  }
}

function ensurePluginScaffoldExists(pluginPath) {
  if (!fs.existsSync(pluginPath)) {
    throw new Error(`目标插件目录不存在：${pluginPath}`);
  }

  const manifestPath = path.join(pluginPath, 'manifest.yaml');
  if (!fs.existsSync(manifestPath)) {
    throw new Error(`缺少 manifest.yaml，请先执行 plugin init：${pluginPath}`);
  }
}

function writeFile(targetPath, content) {
  fs.mkdirSync(path.dirname(targetPath), { recursive: true });
  fs.writeFileSync(targetPath, content, 'utf8');
}

function writeKeepFile(targetPath) {
  writeFile(targetPath, '');
}

function copyTree(sourcePath, targetPath) {
  const stats = fs.statSync(sourcePath);
  if (stats.isDirectory()) {
    fs.mkdirSync(targetPath, { recursive: true });
    for (const entry of fs.readdirSync(sourcePath)) {
      copyTree(path.join(sourcePath, entry), path.join(targetPath, entry));
    }
    return;
  }

  fs.mkdirSync(path.dirname(targetPath), { recursive: true });
  fs.copyFileSync(sourcePath, targetPath);
}

function createArtifactRoot(pluginPath, options = {}) {
  const excludedEntries = new Set(options.excludedEntries || []);
  const includedEntries = options.includedEntries
    ? new Set(options.includedEntries)
    : null;
  const prefix = options.prefix || '1flowbase-plugin-artifact';
  const artifactRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), `${prefix}-${sanitizeCode(getPluginName(pluginPath))}-`)
  );

  for (const entry of fs.readdirSync(pluginPath)) {
    if (includedEntries && !includedEntries.has(entry)) {
      continue;
    }
    if (excludedEntries.has(entry)) {
      continue;
    }
    copyTree(path.join(pluginPath, entry), path.join(artifactRoot, entry));
  }

  return artifactRoot;
}

function createDemoPackageRoot(pluginPath) {
  return createArtifactRoot(pluginPath, {
    prefix: '1flowbase-plugin-demo',
    excludedEntries: ['demo', 'scripts'],
  });
}

function createPackageArtifactRoot(pluginPath) {
  return createArtifactRoot(pluginPath, {
    prefix: '1flowbase-plugin-package',
    includedEntries: [
      '_assets',
      'i18n',
      'manifest.yaml',
      'models',
      'provider',
      'readme',
      'ui',
      'LICENSE',
      'README.md',
      'README.en.md',
    ],
  });
}

function removeDirIfExists(targetPath) {
  if (!targetPath || !fs.existsSync(targetPath)) {
    return;
  }

  fs.rmSync(targetPath, { recursive: true, force: true });
}

function compareStablePath(left, right) {
  if (left === right) {
    return 0;
  }
  return left < right ? -1 : 1;
}

module.exports = {
  assertNonEmptyCode,
  compareStablePath,
  copyTree,
  createArtifactRoot,
  createDemoPackageRoot,
  createPackageArtifactRoot,
  ensurePluginScaffoldExists,
  ensureTargetDirForInit,
  getPluginName,
  log,
  removeDirIfExists,
  sanitizeCode,
  writeFile,
  writeKeepFile,
};
