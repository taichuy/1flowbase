'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

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
  if (tagCommit && tagCommit !== sha) throw new Error('Existing tag points to another commit; refusing to publish mismatched artifacts');
  if (existingRelease && (!existingRelease.isDraft || existingRelease.targetCommitish !== sha)) throw new Error('Only a draft for the same exact candidate can be resumed');
}
function main() {
  const tag = process.env.NATIVE_TAG;
  const sha = process.env.NATIVE_SHA;
  const repo = process.env.GH_REPO;
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag || '') || !/^[0-9a-f]{40}$/.test(sha || '')) throw new Error('Expected a version tag and exact candidate SHA');
  function gh(args) {
    const result = spawnSync('gh', args, { encoding:'utf8', env:process.env });
    if (result.status !== 0) throw new Error(result.stderr || result.error?.message || 'GitHub command failed');
    return result.stdout;
  }
  const json = route => JSON.parse(gh(['api', route]));
  const tagCommit = () => resolveTagCommit(json(`repos/${repo}/git/matching-refs/tags/${encodeURIComponent(tag)}`), tag, id => json(`repos/${repo}/git/tags/${id}`));
  const lookup = spawnSync('gh',['release','view',tag,'--json','isDraft,targetCommitish'],{encoding:'utf8',env:process.env});
  if (lookup.status !== 0 && !/release not found|HTTP 404/i.test(lookup.stderr || '')) throw new Error(lookup.stderr || 'Cannot inspect existing release');
  const existing = lookup.status === 0 ? JSON.parse(lookup.stdout) : null;
  const commit = tagCommit();
  validateReleaseTarget(commit, existing, sha);
  // Claim a new tag explicitly at this SHA. Never rely on --target moving an existing tag.
  if (!commit) gh(['api',`repos/${repo}/git/refs`,'--method','POST','-f',`ref=refs/tags/${tag}`,'-f',`sha=${sha}`]);
  if (!existing) {
    const notes = `Native portable distributions / 原生便携包\n\n- Extract once; edit the root .env; launch 1flowbase.\n- 解压一次，根目录 .env 为运行配置，默认数据目录为 data/。\n- Stop the application, then run upgrade.cmd / upgrade.sh in the current installation to download and apply updates.\n- 双击升级会自动下载并直接更新当前安装，保留 .env 与数据，不创建备份。仅支持同系统、架构与 PostgreSQL 大版本。\n\nSource / 源码: ${sha}\n`;
    const notesPath = 'native-notes.md'; fs.writeFileSync(notesPath, notes);
    gh(['release','create',tag,'--target',sha,'--draft','--prerelease','--title',`${tag} · Native portable distributions`,'--notes-file',notesPath]);
  }
  const directory = 'native-assets';
  const assets = fs.readdirSync(directory).filter(name => /^1flowbase-(linux|macos|windows)-(x64|arm64)\.(zip|tar\.gz)$/.test(name)).map(name => path.join(directory,name));
  if (assets.length !== 5) throw new Error('All five validated native archives are required');
  gh(['release','upload',tag,...assets,'--clobber']);
  validateReleaseTarget(tagCommit(), null, sha);
  if (process.env.NATIVE_PUBLISH === 'true') gh(['release','edit',tag,'--draft=false','--prerelease']);
  console.log(`https://github.com/${repo}/releases/tag/${tag}`);
}
if(require.main===module)try{main();}catch(error){console.error(error.message);process.exitCode=1;}
module.exports={resolveTagCommit,validateReleaseTarget};
