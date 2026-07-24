'use strict';

const fs = require('node:fs');
const path = require('node:path');

function artifactFiles(root) {
  return fs.readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(root, entry.name);
    return entry.isDirectory() ? artifactFiles(target) : [target];
  });
}

function assertNoArtifactSecrets(roots, secrets) {
  const canaries = secrets.filter(Boolean).map((secret) => Buffer.from(secret));
  const scanned = [];
  for (const root of roots.filter((value) => value && fs.existsSync(value))) {
    for (const file of artifactFiles(root)) {
      const bytes = fs.readFileSync(file);
      for (const canary of canaries) {
        if (bytes.includes(canary)) throw new Error(`secret canary leaked into artifact ${file}`);
      }
      scanned.push(file);
    }
  }
  return scanned;
}

module.exports = { artifactFiles, assertNoArtifactSecrets };
