const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const test = require('node:test');
const { scripts } = require('../../../../web/app/package.json');
const { exclusions, pageFilters } = require('../layers.js');
const { parseCliArgs, buildVitestCommand } = require('../../cli/run-frontend-vitest.js');
const { CONTRACT_TEST_FILES } = require('../../test/index.js');

const src = path.resolve(__dirname, '../../../../web/app/src');
const testFiles = new Set(fs.readdirSync(src, { recursive: true })
  .filter((file) => /\.test\.tsx?$/u.test(file))
  .map((file) => `src/${file.split(path.sep).join('/')}`));

function inventory(patterns, filters, files) {
  const excluded = new Set();
  const unmatched = [];
  const paths = [...files];
  for (const pattern of patterns) {
    const matches = pattern.includes('*') ? paths.filter((file) => path.posix.matchesGlob(file, pattern))
      : files.has(pattern) ? [pattern] : [];
    if (!matches.length) unmatched.push(pattern);
    for (const file of matches) excluded.add(file);
  }
  // Vitest positional arguments match substrings (even when they contain a literal '*').
  for (const filter of filters) {
    if (!files.has(filter) && !paths.some((file) => file.includes(filter))) unmatched.push(filter);
  }
  const selected = new Set(filters);
  const missing = [...excluded].filter((file) => !selected.has(file) && !filters.some((filter) => file.includes(filter)));
  return { missing, unmatched };
}

function checkBinding(script, mode, selectors) {
  const prefix = '../../scripts/node/cli/exec-with-real-node.sh ../../scripts/node/cli/run-frontend-vitest.js ';
  assert.ok(script.startsWith(prefix));
  const options = parseCliArgs([...script.slice(prefix.length).split(' '), '--', '-t', 'name filter']);
  assert.equal(options.mode, mode);
  const runtimeConfig = { frontend: { vitestMaxWorkers: 2 } };
  const expected = ['--dir', 'web/app', 'exec', 'vitest', 'run'];
  if (mode === 'coverage') expected.push('--coverage');
  assert.deepEqual(buildVitestCommand({ ...options, runtimeConfig }).args,
    [...expected, '--maxWorkers=2', ...selectors, '-t', 'name filter']);
}

test('package bindings forward the ordered layer before user arguments', () => {
  const args = exclusions.flatMap((pattern) => ['--exclude', pattern]);
  for (const name of ['test', 'test:fast']) checkBinding(scripts[name], 'run', args);
  checkBinding(scripts['test:coverage'], 'coverage', args);
  checkBinding(scripts['test:page-regression'], 'run', pageFilters);
});

test('page-only directories and PR business anchors stay in their designated lanes', () => {
  // Keep expensive suites out of the fast lane without widening page regression to a whole feature.
  for (const [directory, glob] of [
    ['src/features/settings/_tests/model-providers-page/', '*.test.tsx'],
    ['src/features/applications/_tests/application-logs-page/', '**/*.test.tsx'],
  ]) {
    assert.ok(exclusions.includes(directory + glob), directory);
    assert.ok(pageFilters.includes(directory), directory);
  }
  const pr = new Set(scripts['test:pr'].split(' '));
  for (const file of ['src/features/frontstage/_tests/page-tree.test.ts',
    'src/features/agent-flow/_tests/start-node-variables.test.ts']) {
    assert.ok(pr.has(file) && testFiles.has(file), file);
  }
});

test('every selector exists; stable and page suites together cover every targeted contract', () => {
  const result = inventory(exclusions, pageFilters, testFiles);
  for (const file of CONTRACT_TEST_FILES) {
    assert.ok(testFiles.has(file), `contract test must exist: ${file}`);
    assert.ok(!result.missing.includes(file), `contract must execute: ${file}`);
  }
  assert.deepEqual(result, { missing: [], unmatched: [] });
});

test('inventory rejects a literal positional glob, omitted directory and retired paths', () => {
  const dir = 'src/pages/settings/';
  const file = `${dir}roles/policy.test.tsx`;
  const files = new Set([file, 'src/pages/sign-in.test.tsx']);
  const glob = `${dir}**/*.test.tsx`;
  const removed = 'src/pages/retired.test.tsx';
  const moved = 'src/pages/moved.test.tsx';
  assert.deepEqual(inventory([glob], [dir, 'src/pages/sign-in.test.tsx'], files), { missing: [], unmatched: [] });
  assert.deepEqual(inventory([glob], [glob], files), { missing: [file], unmatched: [glob] });
  assert.deepEqual(inventory([glob], [], files).missing, [file]);
  assert.deepEqual(inventory([glob, removed], [dir, moved], files).unmatched, [removed, moved]);
});
