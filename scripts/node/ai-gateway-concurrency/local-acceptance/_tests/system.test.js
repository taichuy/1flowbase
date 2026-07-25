'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const test = require('node:test');

const {
  cleanupTmux,
  requireRepositoryRevision,
  requireRepositoryState,
  requireSourceObject,
} = require('../system');

function git(root, ...args) {
  return execFileSync('git', ['-C', root, ...args], { encoding: 'utf8' }).trim();
}

function repositoryFixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-acceptance-git-'));
  git(root, 'init');
  git(root, 'config', 'user.email', 'qa@example.invalid');
  git(root, 'config', 'user.name', 'QA Fixture');
  fs.writeFileSync(path.join(root, 'tracked.txt'), 'fixed\n');
  git(root, 'add', 'tracked.txt');
  git(root, 'commit', '-m', 'fixture');
  return root;
}

test('AC-028 controlled negative: dirty project worktrees fail closed', () => {
  const root = repositoryFixture();
  try {
    const revision = git(root, 'rev-parse', 'HEAD');
    assert.equal(requireRepositoryState('fixture', { path: root, revision }).revision, revision);
    fs.appendFileSync(path.join(root, 'tracked.txt'), 'dirty\n');
    assert.throws(
      () => requireRepositoryState('fixture', { path: root, revision }),
      /must be clean/u,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-029: protected baseline verifies revision without owning private worktree cleanliness', () => {
  const root = repositoryFixture();
  try {
    const revision = git(root, 'rev-parse', 'HEAD');
    fs.appendFileSync(path.join(root, 'tracked.txt'), 'private-memory-change\n');
    assert.deepEqual(requireRepositoryRevision('protected', { path: root, revision }), {
      name: 'protected', path: root, revision, clean: null,
    });
    assert.throws(
      () => requireRepositoryRevision('protected', { path: root, revision: 'f'.repeat(40) }),
      /revision mismatch/u,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-028 controlled negative: missing local source objects fail before detached worktree creation', () => {
  const root = repositoryFixture();
  try {
    assert.throws(
      () => requireSourceObject('fixture', { repository: root, revision: 'f'.repeat(40) }),
      /local source object check failed/u,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-028: cleanup removes owned stale tmux sockets without touching foreign sockets', async () => {
  const socketRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'local-acceptance-tmux-'));
  const ownedSocket = 'owned-stale';
  const foreignSocket = 'foreign-stale';
  fs.writeFileSync(path.join(socketRoot, ownedSocket), '');
  fs.writeFileSync(path.join(socketRoot, foreignSocket), '');
  const killAttempts = [];
  try {
    await cleanupTmux({
      socketRoot,
      prefix: 'owned-',
      killServer(socket) { killAttempts.push(socket); },
    });
    assert.deepEqual(killAttempts, [ownedSocket]);
    assert.equal(fs.existsSync(path.join(socketRoot, ownedSocket)), false);
    assert.equal(fs.existsSync(path.join(socketRoot, foreignSocket)), true);
  } finally {
    fs.rmSync(socketRoot, { recursive: true, force: true });
  }
});

test('AC-028 controlled negative: cleanup fails closed when an owned tmux socket remains', async () => {
  const socketRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'local-acceptance-tmux-'));
  const ownedSocket = 'owned-residue';
  fs.writeFileSync(path.join(socketRoot, ownedSocket), '');
  try {
    await assert.rejects(
      cleanupTmux({
        socketRoot,
        prefix: 'owned-',
        killServer() {},
        removeSocket() { throw new Error('fixture removal failure'); },
      }),
      /owned tmux cleanup left residue: owned-residue/u,
    );
  } finally {
    fs.rmSync(socketRoot, { recursive: true, force: true });
  }
});
