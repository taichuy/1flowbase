const fs = require('node:fs');
const path = require('node:path');
const manifest = require('./manifest.json');

function parseList(text) {
  return text.split(/\r?\n/u).flatMap(line => line.endsWith(': test') ? [line.slice(0, -6)] : []);
}
function selectTests(target, listed, required = manifest.required) {
  if (new Set(listed).size !== listed.length) throw new Error(`${target.id}: duplicate --list names`);
  const must = required.filter(entry => entry.target === target.id);
  for (const entry of must) {
    if (entry.expected !== 1 || listed.filter(name => name === entry.name).length !== entry.expected) {
      throw new Error(`${target.id}: missing/non-unique required test ${entry.name}`);
    }
  }
  for (const filter of target.regressionFilters) {
    if (!listed.some(name => name.includes(filter))) throw new Error(`${target.id}: zero regression filter ${filter}`);
  }
  for (const name of listed.filter(name => name.split('::').at(-1).startsWith('root_2007_'))) {
    if (!must.some(entry => entry.name === name)) throw new Error(`${target.id}: unmapped Root test ${name}`);
  }
  const chosen = new Set(must.map(entry => entry.name));
  for (const name of listed) if (target.regressionFilters.some(filter => name.includes(filter))) chosen.add(name);
  if (chosen.size < target.minimum) throw new Error(`${target.id}: zero selected tests`);
  return [...chosen].sort();
}
function passedExact(text, name, code) {
  return code === 0 && text.split(/\r?\n/u).includes(`test ${name} ... ok`)
    && /test result: ok\. 1 passed; 0 failed; 0 ignored;/u.test(text);
}
function validateSources(root, data = manifest) {
  const requiredKeys = new Set();
  for (const entry of data.required) {
    const key = `${entry.target}/${entry.name}`;
    if (requiredKeys.has(key)) throw new Error(`duplicate manifest entry ${key}`);
    requiredKeys.add(key);
    const source = fs.readFileSync(path.join(root, entry.source), 'utf8');
    const leaf = entry.name.split('::').at(-1);
    if (!new RegExp(`\\bfn\\s+${leaf}\\s*\\(`, 'u').test(source)) throw new Error(`source test missing: ${key}`);
  }
  // Every source-defined Root test must belong to one manifest target; --list later verifies
  // actual Rust module paths, cfg gates and linker/test-target inclusion.
  const seen = new Set();
  function walk(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      if (entry.name === 'target') continue;
      const filename = path.join(directory, entry.name);
      if (entry.isDirectory()) walk(filename);
      else if (entry.name.endsWith('.rs')) {
        const source = path.relative(root, filename).split(path.sep).join('/');
        for (const match of fs.readFileSync(filename, 'utf8').matchAll(/\bfn\s+(root_2007_\w+)\s*\(/gu)) {
          const mapped = data.required.filter(row => row.source === source && row.name.split('::').at(-1) === match[1]);
          if (mapped.length !== 1) throw new Error(`unmapped/non-unique Root source ${source}:${match[1]}`);
          seen.add(`${mapped[0].target}/${mapped[0].name}`);
        }
      }
    }
  }
  for (const packageRoot of new Set(data.targets.map(target => target.packageRoot))) {
    for (const child of ['src', 'tests']) {
      const directory = path.join(root, packageRoot, child);
      if (fs.existsSync(directory)) walk(directory);
    }
  }
  if (seen.size !== data.required.length) throw new Error('manifest/source inventory mismatch');
  for (const prefix of ['AC', 'AUTH']) for (let index = 1; index <= 10; index++) {
    const id = `${prefix}-${String(index).padStart(prefix === 'AC' ? 3 : 2, '0')}`;
    if (!data.required.some(row => row[prefix === 'AC' ? 'ac' : 'auth'].includes(id))) throw new Error(`unmapped ${id}`);
  }
}
module.exports = { manifest, parseList, selectTests, passedExact, validateSources };
