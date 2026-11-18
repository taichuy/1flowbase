const fs = require('node:fs');
const path = require('node:path');

// API coverage remains one aggregate as its production owners move out of Root.
// Only manifests that actually exist become Cargo selections.
const API_SOURCE_OWNERS = Object.freeze([
  { packageName: 'api-server', directory: 'api/apps/api-server' },
  { packageName: 'api-host-kernel', directory: 'api/crates/api/kernel' },
  { packageName: 'api-public-gateway', directory: 'api/crates/api/public-gateway' },
  { packageName: 'api-applications', directory: 'api/crates/api/applications' },
  { packageName: 'api-console-settings', directory: 'api/crates/api/console-settings' },
  { packageName: 'api-mcp', directory: 'api/crates/api/mcp' },
  { packageName: 'api-plugins-models', directory: 'api/crates/api/plugins-models' },
  { packageName: 'api-console-interaction', directory: 'api/crates/api/console-interaction' },
].map((owner) => Object.freeze({ ...owner, sourcePathMarker: `/${owner.directory}/src/` })));

function apiSourceOwners(repoRoot) {
  return API_SOURCE_OWNERS.filter((owner, index) => index === 0
    || fs.existsSync(path.join(repoRoot, owner.directory, 'Cargo.toml')));
}

function apiPackageArgs(repoRoot) {
  return apiSourceOwners(repoRoot).flatMap(({ packageName }) => ['--package', packageName]);
}

module.exports = { API_SOURCE_OWNERS, apiSourceOwners, apiPackageArgs };
