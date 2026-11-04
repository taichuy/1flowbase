// Test-only TypeScript loading bridge. All compilation policy is the actual Host compiler.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');
const root = process.argv[2];
const webRequire = createRequire(path.join(root, 'web/package.json'));
const ts = webRequire('typescript');
require.extensions['.ts'] = (module, filename) => {
  const source = fs.readFileSync(filename, 'utf8');
  const output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
  });
  module._compile(output.outputText, filename);
};
const { compileNativeReactComponent } = require(path.join(
  root, 'web/packages/page-runtime/src/native-react-compiler/component-compiler.ts'
));
const definitions = [
  { module_source: 'react', exports: ['useState'] },
  { module_source: 'react/jsx-runtime', exports: ['Fragment', 'jsx', 'jsxs'] }
];
const sources = JSON.parse(fs.readFileSync(0, 'utf8'));
for (const source of sources) {
  const result = compileNativeReactComponent(source, definitions);
  assert.equal(result.ok, true, JSON.stringify(result.diagnostics));
}
// Controlled negative: the previous projection would synthesize this duplicate import.
const duplicate = compileNativeReactComponent(
  `import { useState } from 'react';\n${sources[0]}`, definitions
);
assert.equal(duplicate.ok, false, 'fixture must reject duplicate imports from the old projection');
process.stdout.write(JSON.stringify({ compiled: sources.length, duplicate_import_rejected: true }));
