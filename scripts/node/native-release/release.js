'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const ASSETS = [
  '1flowbase-linux-arm64.tar.gz', '1flowbase-linux-x64.tar.gz',
  '1flowbase-macos-arm64.tar.gz', '1flowbase-macos-x64.tar.gz', '1flowbase-windows-x64.zip',
];
function resolveTagCommit(refs, tag, loadTag) {
  let object = refs.find(ref => ref.ref === `refs/tags/${tag}`)?.object;
  if (!object) return null;
  for (let depth = 0; depth < 16; depth++) {
    if (object.type === 'commit') return object.sha;
    if (object.type !== 'tag') throw new Error('Release tag must resolve to a commit');
    object = loadTag(object.sha).object;
  }
  throw new Error('Annotated tag chain is too deep');
}
function validateReleaseTarget(tagCommit, existingRelease, sha) {
  if (tagCommit && tagCommit !== sha) throw new Error('Existing tag points to another commit; refusing mismatched artifacts');
  if (existingRelease?.isDraft && existingRelease.targetCommitish !== sha) throw new Error('Only a draft for the same exact candidate can be resumed');
  if (existingRelease && !existingRelease.isDraft && tagCommit !== sha) throw new Error('Published release must have a tag at the exact candidate');
}
function releaseNotes({ kind = 'native', tag, sha, repo, server = 'https://github.com', runId = '' }) {
  const native = `## Native portable distributions / 原生便携包

Download the archive for your OS and architecture below. Extract once; edit the root .env; launch 1flowbase.
下载下方对应系统/架构的包，解压一次，根目录 .env 为运行配置。
Stop the application, then run upgrade.cmd / upgrade.sh in the current installation to download and apply updates.
双击升级会自动下载并直接更新当前安装，保留 .env 与数据，不创建备份。仅支持同系统、架构与 PostgreSQL 大版本。
`;
  if (kind === 'native') return `${native}\nSource / 源码: ${sha}\n`;
  return `## Installation or Upgrade

Linux/macOS:

\`\`\`bash
curl -fsSL https://raw.githubusercontent.com/${repo}/main/scripts/shell/docker-deploy.sh | sh
\`\`\`

Windows PowerShell:

\`\`\`powershell
irm https://raw.githubusercontent.com/${repo}/main/scripts/powershell/docker-deploy.ps1 | iex
\`\`\`

Windows CMD:

\`\`\`cmd
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/${repo}/main/scripts/powershell/docker-deploy.ps1 | iex"
\`\`\`

## Docker Images

- \`ghcr.io/${repo.split('/')[0]}/1flowbase-web:${tag}\`
- \`ghcr.io/${repo.split('/')[0]}/1flowbase-api-server:${tag}\`
- \`ghcr.io/${repo.split('/')[0]}/1flowbase-*:latest\`

${native}
## Traceability

- Target commit: [\`${sha.slice(0, 7)}\`](${server}/${repo}/commit/${sha})
- Workflow run: [container images #${runId}](${server}/${repo}/actions/runs/${runId})
- Source branch: \`latest\`

Pull requests, linked issues, commits, and contributors are generated below by GitHub when available.
`;
}
function publishRelease(options, run = spawnSync) {
  const { tag, sha, repo, directory, publish, kind = 'native' } = options;
  if (!['native', 'combined'].includes(kind)) throw new Error('Unknown release kind');
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag || '') || !/^[0-9a-f]{40}$/.test(sha || '') || !/^[\w.-]+\/[\w.-]+$/.test(repo || '')) throw new Error('Expected a version tag, repository and exact candidate SHA');
  const prerelease = kind === 'native';
  if (!prerelease && !/^v\d+\.\d+\.\d+$/.test(tag)) throw new Error('Combined Docker releases require a stable version');
  const names = fs.readdirSync(directory).filter(name => name.startsWith('1flowbase-')).sort();
  if (JSON.stringify(names) !== JSON.stringify(ASSETS) || names.some(name => !fs.lstatSync(path.join(directory, name)).isFile() || fs.statSync(path.join(directory, name)).size === 0)) throw new Error('All five validated native archives are required');
  const invoke = args => run('gh', args, { encoding: 'utf8', env: process.env });
  function gh(args) {
    const result = invoke(args);
    if (result.status !== 0) throw new Error(result.stderr || result.error?.message || 'GitHub command failed');
    return result.stdout;
  }
  const json = route => JSON.parse(gh(['api', route]));
  const tagCommit = () => resolveTagCommit(json(`repos/${repo}/git/matching-refs/tags/${encodeURIComponent(tag)}`), tag, id => json(`repos/${repo}/git/tags/${id}`));
  // gh release view combines REST (published releases) and GraphQL (drafts).
  const lookup = invoke(['release', 'view', tag, '--json', 'isDraft,targetCommitish,isPrerelease,assets']);
  if (lookup.status !== 0 && !/HTTP 404|Not Found|release not found/i.test(lookup.stderr || '')) throw new Error(lookup.stderr || 'Cannot inspect existing release');
  const release = lookup.status === 0 ? JSON.parse(lookup.stdout) : null;
  const existing = release;
  const commit = tagCommit();
  validateReleaseTarget(commit, existing, sha);
  if (release && release.isPrerelease !== prerelease) throw new Error('Existing release has a different release channel');
  if (release && !release.isDraft) {
    if (!ASSETS.every(name => release.assets?.some(asset => asset.name === name && asset.state === 'uploaded' && asset.size > 0))) throw new Error('Published release is missing native assets; refusing to rewrite it');
    return { existing: true, url: `https://github.com/${repo}/releases/tag/${tag}` };
  }
  if (!commit) gh(['api', `repos/${repo}/git/refs`, '--method', 'POST', '-f', `ref=refs/tags/${tag}`, '-f', `sha=${sha}`]);
  if (!release) {
    const notesPath = path.join(directory, '.release-notes.md');
    fs.writeFileSync(notesPath, releaseNotes(options));
    const args = ['release', 'create', tag, '--target', sha, '--draft', '--title', kind === 'combined' ? tag : `${tag} · Native portable distributions`, '--notes-file', notesPath];
    if (prerelease) args.push('--prerelease');
    if (kind === 'combined') args.push('--generate-notes');
    gh(args);
  }
  gh(['release', 'upload', tag, ...ASSETS.map(name => path.join(directory, name)), '--clobber']);
  // Recheck the tag after upload and before making the complete release visible.
  if (tagCommit() !== sha) throw new Error('Release tag changed or disappeared during upload');
  if (publish) gh(['release', 'edit', tag, '--draft=false', `--prerelease=${prerelease}`]);
  return { existing: false, url: `https://github.com/${repo}/releases/tag/${tag}` };
}
function main() {
  const env = process.env;
  const kind = env.NATIVE_RELEASE_KIND || 'native';
  if (kind === 'combined' && env.NATIVE_TAG !== `v${fs.readFileSync(path.resolve(__dirname, '../../../VERSION'), 'utf8').trim()}`) throw new Error('Release tag differs from VERSION');
  const result = publishRelease({ tag: env.NATIVE_TAG, sha: env.NATIVE_SHA, repo: env.GH_REPO, directory: 'native-assets', publish: env.NATIVE_PUBLISH === 'true', kind, server: env.GITHUB_SERVER_URL, runId: env.GITHUB_RUN_ID });
  console.log(`${result.existing ? 'Already published: ' : ''}${result.url}`);
}
if (require.main === module) try { main(); } catch (error) { console.error(error.message); process.exitCode = 1; }
module.exports = { ASSETS, resolveTagCommit, validateReleaseTarget, releaseNotes, publishRelease };
