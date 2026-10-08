import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { execFileSync } from 'node:child_process';

// Source-integration fixture: only Linux uses a previously verified real binary.
// Other archives are opaque fixture bytes, not cross-platform release evidence.
export async function createRemoteFixture({ pluginsRoot, directory, nativeBinary }) {
  const { listCollectors, PLATFORM_TARGETS, archiveName } = await import(pathToFileURL(path.join(pluginsRoot, 'scripts/collectors/catalog.mjs')));
  const { buildDistribution } = await import(pathToFileURL(path.join(pluginsRoot, 'scripts/collectors/distribution.mjs')));
  const { updateCategoryCatalog } = await import(pathToFileURL(path.join(pluginsRoot, 'scripts/extension-catalog.mjs')));
  const collector = listCollectors(pluginsRoot).find(item => item.collector_code === 'codex-logs-collector');
  const nativeDirectory = path.join(directory, 'native');
  fs.mkdirSync(nativeDirectory, { recursive: true });
  for (const platform of PLATFORM_TARGETS) fs.writeFileSync(path.join(nativeDirectory, archiveName(collector, platform)), `fixture opaque ${platform.rust_target}`);
  execFileSync(process.execPath, [path.join(pluginsRoot, 'scripts/collectors/package.mjs'), collector.collector_code, 'x86_64-unknown-linux-musl', nativeBinary, nativeDirectory], { cwd: pluginsRoot, stdio: 'ignore' });
  const { privateKey, publicKey } = crypto.generateKeyPairSync('ed25519');
  const { archive, entry, manifest } = buildDistribution({ repoRoot: pluginsRoot, collector, nativeDirectory,
    outputDirectory: path.join(directory, 'distribution'), sourceSha: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: pluginsRoot, encoding: 'utf8' }).trim(), privateKey, keyId: 'distribution-proof-key' });
  const requests = [];
  let disabled = false;
  const server = http.createServer((request, response) => {
    requests.push({ path: request.url, disabled });
    if (disabled) { response.writeHead(503); response.end('fixture remote disabled'); return; }
    const url = new URL(request.url, 'http://fixture.invalid');
    const file = url.pathname === '/distribution.tar.gz' ? archive : path.join(directory, 'catalog', url.pathname.replace(/^\//, ''));
    if (!file.startsWith(path.join(directory, 'catalog') + path.sep) && file !== archive) { response.writeHead(404); response.end(); return; }
    if (!fs.existsSync(file) || !fs.statSync(file).isFile()) { response.writeHead(404); response.end(); return; }
    response.writeHead(200, { 'content-type': file.endsWith('.json') ? 'application/json' : 'application/octet-stream' });
    fs.createReadStream(file).pipe(response);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const baseUrl = `http://127.0.0.1:${server.address().port}`;
  entry.download_locator.locator = `${baseUrl}/distribution.tar.gz`;
  const source = path.join(directory, 'catalog', collector.plugin_dir);
  fs.mkdirSync(source, { recursive: true });
  fs.copyFileSync(path.join(pluginsRoot, collector.plugin_dir, 'collector-manifest.json'), path.join(source, 'collector-manifest.json'));
  fs.writeFileSync(path.join(source, 'catalog-entry.json'), JSON.stringify(entry));
  updateCategoryCatalog({ repoRoot: path.join(directory, 'catalog'), category: 'runtime-extensions', rawBaseUrl: baseUrl });
  // A local source-integration fixture must not inherit production remote locators.
  const catalogRoot = path.join(directory, 'catalog', 'runtime-extensions', 'catalog', 'v1');
  function checkLocators(value, key = '') {
    if (typeof value === 'string' && /^https?:/.test(value) && (key === 'locator' || key.endsWith('_locator')) && !value.startsWith(baseUrl + '/')) {
      throw new Error(`Fixture locator escaped its local server: ${key}`);
    }
    if (value && typeof value === 'object') for (const [name, child] of Object.entries(value)) checkLocators(child, name);
  }
  try {
    for (const file of fs.readdirSync(catalogRoot, { recursive: true })) {
      if (file.endsWith('.json')) checkLocators(JSON.parse(fs.readFileSync(path.join(catalogRoot, file), 'utf8')));
    }
  } catch (error) {
    await new Promise(resolve => server.close(resolve));
    throw error;
  }
  fs.writeFileSync(path.join(directory, 'catalog', 'official-registry.json'), JSON.stringify({ schema_version: '1flowbase.official-plugin-registry/v1', plugins: [] }));
  return { baseUrl, archive, manifest, collector, requests,
    trustedKeys: JSON.stringify([{ key_id: 'distribution-proof-key', algorithm: 'ed25519', public_key_pem: publicKey.export({ type: 'spki', format: 'pem' }) }]),
    disable() { disabled = true; }, close: () => new Promise(resolve => server.close(resolve)) };
}
