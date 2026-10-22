const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const test = require('node:test');

const appRoot = path.resolve(__dirname, '../../../../web/app');
const { scripts } = JSON.parse(fs.readFileSync(path.join(appRoot, 'package.json'), 'utf8'));
const testFiles = fs.readdirSync(path.join(appRoot, 'src'), { recursive: true })
  .filter((file) => /\.test\.tsx?$/u.test(file))
  .map((file) => `src/${file.split(path.sep).join('/')}`);

function tokens(command) {
  return Array.from(command.matchAll(/'([^']*)'|"([^"]*)"|(\S+)/gu),
    (match) => match[1] ?? match[2] ?? match[3]);
}

function missingPageCoverage(script, pageScript) {
  const args = tokens(script);
  const exclusions = args.flatMap((arg, index) => arg === '--exclude' ? [args[index + 1]] : []);
  // Vitest positional filters are substrings, unlike --exclude globs.
  const pageFilters = tokens(pageScript).filter((arg) => arg.startsWith('src/'));
  return testFiles.filter((file) => exclusions.some((glob) => path.posix.matchesGlob(file, glob))
    && !pageFilters.some((filter) => file.includes(filter)));
}

test('frontend run and coverage exclusions are all selected by page regression', () => {
  for (const name of ['test', 'test:fast', 'test:coverage']) {
    assert.deepEqual(missingPageCoverage(scripts[name], scripts['test:page-regression']), [], name);
  }
});

test('repository full frontend covers every targeted contract through normal and page suites', () => {
  const { CONTRACT_TEST_FILES } = require('../../test/index.js');
  const missing = missingPageCoverage(scripts.test, scripts['test:page-regression']);
  for (const file of CONTRACT_TEST_FILES) {
    assert.ok(testFiles.includes(file), `contract test must exist: ${file}`);
    assert.ok(!missing.includes(file), `contract must execute in the full frontend lane: ${file}`);
  }
});

test('inventory rejects quoted positional globs and omitted page directories', () => {
  const directory = 'src/features/settings/_tests/model-providers-page/';
  for (const replacement of [`'${directory}*.test.tsx'`, '']) {
    const broken = scripts['test:page-regression'].replace(directory, replacement);
    const missing = missingPageCoverage(scripts.test, broken);
    assert.ok(missing.length > 0);
    assert.ok(missing.every((file) => file.startsWith(directory)));
  }
});


function unmatchedSelectors(script, pageScript, files = testFiles) {
  const args = tokens(script);
  const exclusions = args.flatMap((arg, index) => arg === '--exclude' ? [args[index + 1]] : []);
  const pageFilters = tokens(pageScript).filter((arg) => arg.startsWith('src/'));
  return [
    ...exclusions.filter((glob) => !files.some((file) => path.posix.matchesGlob(file, glob))),
    ...pageFilters.filter((filter) => !files.some((file) => file.includes(filter))),
  ];
}

test('every frontend lane selector targets an existing test rather than borrowing adjacent execution evidence', () => {
  for (const name of ['test', 'test:fast', 'test:coverage']) {
    assert.deepEqual(unmatchedSelectors(scripts[name], scripts['test:page-regression']), [], name);
  }
});

test('lane inventory rejects a removed test and accepts nested scenario globs', () => {
  const files = ['src/pages/settings/roles/policy.test.tsx', 'src/pages/sign-in.test.tsx'];
  assert.deepEqual(unmatchedSelectors(
    "run --exclude 'src/pages/settings/**/*.test.tsx'",
    'run src/pages/settings/ src/pages/sign-in.test.tsx', files,
  ), []);
  assert.deepEqual(unmatchedSelectors(
    "run --exclude 'src/pages/settings/**/*.test.tsx' --exclude src/pages/retired.test.tsx",
    'run src/pages/settings/ src/pages/moved.test.tsx', files,
  ), ['src/pages/retired.test.tsx', 'src/pages/moved.test.tsx']);
});
