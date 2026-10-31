'use strict';
const fs = require('node:fs');
const path = require('node:path');

const TARGETS = ['linux-x64', 'linux-arm64', 'macos-x64', 'macos-arm64', 'windows-x64', 'all'];
function validateBuildInputs({ target, tag = '', publish = false }) {
  if (!TARGETS.includes(target)) throw new Error('Unsupported native target');
  if (tag && (target !== 'all' || !/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag))) throw new Error('Release requires all targets and a valid version tag');
  if (publish && !tag) throw new Error('Publishing requires release_tag');
}
function selectContainerRelease({ eventName, ref, webEnabled, apiEnabled, webTag, apiTag, version, manualCombined = false, promoteOfficialTags = true }) {
  const manual = eventName === 'workflow_dispatch' && manualCombined;
  if (eventName !== 'push' && !manual) return { enabled: false, tag: '' };
  if (ref !== 'refs/heads/latest') {
    if (manual) throw new Error('Complete publication must run from the latest branch');
    return { enabled: false, tag: '' };
  }
  if (manual && (!webEnabled || !apiEnabled || !promoteOfficialTags)) {
    throw new Error('Complete publication requires both components and official tag promotion');
  }
  if (!/^\d+\.\d+\.\d+$/.test(version) || webTag !== `v${version}` || apiTag !== `v${version}`) {
    throw new Error('Official releases require VERSION, web and API versions to match; use bump-version');
  }
  return { enabled: webEnabled || apiEnabled, tag: `v${version}` };
}
if (require.main === module) {
  try {
    const env = process.env;
    const plan = selectContainerRelease({
      eventName: env.RELEASE_EVENT, ref: env.RELEASE_REF,
      webEnabled: env.RELEASE_WEB_ENABLED === 'true', apiEnabled: env.RELEASE_API_ENABLED === 'true',
      webTag: env.RELEASE_WEB_TAG, apiTag: env.RELEASE_API_TAG,
      manualCombined: env.RELEASE_MANUAL_COMBINED === 'true',
      promoteOfficialTags: env.RELEASE_PROMOTE_OFFICIAL_TAGS === 'true',
      version: fs.readFileSync(path.resolve(__dirname, '../../../VERSION'), 'utf8').trim(),
    });
    fs.appendFileSync(env.GITHUB_OUTPUT, `enabled=${plan.enabled}\ntag=${plan.tag}\n`);
    console.log(plan.enabled ? `Docker and native release: ${plan.tag}` : 'No combined version release selected');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
module.exports = { validateBuildInputs, selectContainerRelease };
