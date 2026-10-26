#!/usr/bin/env node
'use strict';
// Reuse Docker's pinned bootstrap sources; no second version policy for native releases.
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
function run(program, args, options = {}) {
  const result = spawnSync(program, args, { stdio: 'inherit', ...options });
  if (result.error || result.status !== 0) throw result.error || new Error(`${program} failed (${result.status})`);
  return result.stdout;
}
function prepareResources(output, localRepository) {
  const repoRoot = path.resolve(__dirname, '../../..');
  const dockerfile = fs.readFileSync(path.join(repoRoot, 'docker/api-server.Dockerfile'), 'utf8');
  const arg = name => { const value = dockerfile.match(new RegExp(`^ARG ${name}=(.+)$`, 'm'))?.[1]; if (!value) throw new Error(`Missing Docker bootstrap pin: ${name}`); return value; };
  output = path.resolve(output);
  if (fs.existsSync(output)) throw new Error('Bootstrap output already exists');
  fs.mkdirSync(output, { recursive: true });
  const pricingRepository = arg('MODEL_PRICING_REPOSITORY');
  const pricingRef = arg('MODEL_PRICING_REF');
  const templateRepository = arg('APPLICATION_TEMPLATE_REPOSITORY');
  const templateRef = arg('APPLICATION_TEMPLATE_REF');
  const env = { ...process.env };
  if (localRepository) {
    localRepository = path.resolve(localRepository);
    run('git', ['-C', localRepository, 'cat-file', '-e', pricingRef]);
    run('git', ['-C', localRepository, 'cat-file', '-e', templateRef]);
    env.GIT_CONFIG_COUNT = '1';
    env.GIT_CONFIG_KEY_0 = `url.${localRepository}/.insteadOf`;
    env.GIT_CONFIG_VALUE_0 = `https://github.com/${pricingRepository}.git`;
  }
  run('sh', [path.join(repoRoot, 'scripts/shell/package-model-pricing-bootstrap.sh'), pricingRepository, pricingRef, path.join(output, 'model-pricing')], { env });
  run('sh', [path.join(repoRoot, 'scripts/shell/package-application-templates.sh'), localRepository || templateRepository, templateRef, path.join(output, 'application-templates')]);
  // Local archive preparation uses the exact pinned tree; keep portable source provenance.
  if (localRepository) {
    const receiptPath = path.join(output, 'application-templates/receipt.json');
    const receipt = JSON.parse(fs.readFileSync(receiptPath, 'utf8'));
    receipt.repository = templateRepository;
    receipt.published_download_verified = false;
    fs.writeFileSync(receiptPath, JSON.stringify(receipt, null, 2) + '\n');
  }
  return output;
}
if (require.main === module) {
  try { console.log(prepareResources(process.argv[2], process.argv[3])); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
module.exports = { prepareResources };
