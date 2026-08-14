const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');

test('deployment no longer packages or invokes the removed frontstage executable upgrade', () => {
  for (const relativePath of [
    'docker/api-server.Dockerfile',
    'docker/docker-compose.yaml',
    'docker/docker-compose.external-db.yaml',
    'docker/docker-compose.dev.yaml',
    'scripts/shell/docker-deploy.sh',
    'scripts/powershell/docker-deploy.ps1',
    '.github/workflows/container-images.yml',
  ]) {
    const source = read(relativePath);
    assert.doesNotMatch(source, /frontstage[_-]executable[_-]upgrade/iu, relativePath);
  }
});
