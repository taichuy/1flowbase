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
  const patterns = exclusions.map((glob) => new RegExp(`^${glob.split('*')
    .map((part) => part.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')).join('[^/]*')}$`, 'u'));
  // Vitest positional filters are substrings, unlike --exclude globs.
  const pageFilters = tokens(pageScript).filter((arg) => arg.startsWith('src/'));
  return testFiles.filter((file) => patterns.some((pattern) => pattern.test(file))
    && !pageFilters.some((filter) => file.includes(filter)));
}

test('frontend run and coverage exclusions are all selected by page regression', () => {
  for (const name of ['test', 'test:fast', 'test:coverage']) {
    assert.deepEqual(missingPageCoverage(scripts[name], scripts['test:page-regression']), [], name);
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
