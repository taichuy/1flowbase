const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const viteConfigPath = path.resolve(__dirname, '..', '..', '..', '..', 'web', 'app', 'vite.config.ts');

test('vite config uses the repo default frontend port', () => {
  const viteConfigSource = fs.readFileSync(viteConfigPath, 'utf8');

  assert.match(viteConfigSource, /server:\s*\{/u);
  assert.match(viteConfigSource, /host:\s*'0\.0\.0\.0'/u);
  assert.match(viteConfigSource, /VITE_DEV_SERVER_PORT/u);
  assert.match(viteConfigSource, /VITE_DEV_ALLOWED_HOSTS/u);
  assert.match(viteConfigSource, /Number\.parseInt/u);
  assert.match(viteConfigSource, /3100/u);
  assert.match(viteConfigSource, /strictPort:\s*true/u);
});

test('vite config keeps the workspace root while extending fs allow list for shared scripts', () => {
  const viteConfigSource = fs.readFileSync(viteConfigPath, 'utf8');

  assert.match(viteConfigSource, /searchForWorkspaceRoot\(process\.cwd\(\)\)/u);
  assert.match(viteConfigSource, /new URL\('\.\.\/\.\.\/scripts', import\.meta\.url\)/u);
});
