const fs = require('node:fs');
const path = require('node:path');

// Read a production owner and all of its private modules, including future splits.
// Deliberate negative fixtures and test-only modules are outside this boundary.
function productionRustSources(root) {
  if (!fs.existsSync(root)) throw new Error(`Production source owner is missing: ${root}`);
  if (fs.statSync(root).isFile()) {
    if (path.basename(root) === 'mod.rs') return productionRustSources(path.dirname(root));
    const children = root.replace(/\.rs$/u, '');
    return fs.readFileSync(root, 'utf8') + '\n' + (children && fs.existsSync(children) ? productionRustSources(children) : '');
  }
  return fs.readdirSync(root, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))
    .filter((entry) => !['_tests', 'tests', 'target', 'tests.rs'].includes(entry.name))
    .flatMap((entry) => {
      const file = path.join(root, entry.name);
      if (entry.isDirectory()) return [productionRustSources(file)];
      return entry.isFile() && path.extname(file) === '.rs' ? [fs.readFileSync(file, 'utf8')] : [];
    }).join('\n');
}

function bootRustSources(repoRoot) {
  const root = path.join(repoRoot, 'api/apps/api-server/src');
  return productionRustSources(path.join(root, 'lib.rs')) + '\n' + productionRustSources(path.join(root, 'internal'));
}

module.exports = { productionRustSources, bootRustSources };
