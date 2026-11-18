const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { REVIEW_FILE, sourceDigest, evaluateDiagnosticReviews } = require('../../diagnostic-review.js');

const TOOL = 'react-doctor@0.2.16';
const EVENT = 'no-event-handler';
const EVENT_MESSAGE = 'Faking an event handler with a prop plus a useEffect costs an extra render & runs late.';
const MUTATION = 'query-mutation-missing-invalidation';
const MUTATION_MESSAGE = 'useMutation with no cache update leaves your users looking at stale data after it runs.';
const semanticTest = (name, run) => test(`deferred native provenance: ${name}`, run);

function fixture(t, kind = 'history', options = {}) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'doctor-deferred-native-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const file = 'web/app/src/owner.tsx';
  const sources = new Map();
  const write = (name, source) => {
    fs.mkdirSync(path.dirname(path.join(repoRoot, name)), { recursive: true });
    fs.writeFileSync(path.join(repoRoot, name), source);
    sources.set(name, source);
  };
  const count = options.count ?? 1;
  const targets = [];
  const declarations = [];
  for (let index = 0; index < count; index++) {
    const component = `Panel${index}`;
    const target = `web/app/src/panel-${index}.tsx`;
    const exported = options.bare
      ? `export default function ${component}() { return null; }`
      : `export function ${component}() { return null; }`;
    write(target, `${exported}\n`);
    targets.push({ file: target, component, exported });
    const imported = `import('./panel-${index}')`;
    const loader = options.loader ?? `() => ${options.bare ? imported
      : `${imported}.then(module => ({ default: module.${component} }))`}`;
    declarations.push(`const Deferred${index} = ${options.callee ?? 'lazy'}(${loader});`);
  }
  const query = kind !== 'form';
  const imports = `${options.reactImport ?? "import React, { lazy, useEffect } from 'react';"}
${query ? "import { useQueryClient, useMutation, QueryClient } from '@tanstack/react-query';" : "import { Form } from 'antd';"}
${kind === 'history' ? "import { recordsKey } from './records-key';" : ''}`;
  const receiver = query ? 'const client = useQueryClient();' : 'const [editor] = Form.useForm();';
  const behavior = kind === 'query'
    ? "useMutation({ mutationFn: save, onSuccess: () => client.invalidateQueries({ queryKey: ['records'] }) });"
    : kind === 'form'
      ? 'useEffect(() => { editor.setFieldsValue(receipt); }, [receipt, editor]);'
      : 'useEffect(() => { if (ready) { void client.invalidateQueries({ queryKey: recordsKey() }); } }, [ready, client]);';
  const views = Array.from({ length: count }, (_, index) => `<Deferred${index} />`).join('');
  const consumer = kind === 'form' ? `<Form form={editor}>${views}</Form>` : `<>${views}</>`;
  const loaderDeclarations = declarations.join('\n');
  write(file, `${imports}
${options.prefix ?? ''}
${options.inside ? '' : loaderDeclarations}
function Owner({ ready, receipt, save }) {
  ${receiver}
  ${options.inside ? loaderDeclarations : ''}
  ${behavior}
  return ${consumer};
}
`);
  if (kind === 'history') write('web/app/src/records-key.ts', "export const recordsKey = () => ['records'];\n");
  const manifest = { schemaVersion: 1, toolPackage: TOOL, reviews: [] };
  write(REVIEW_FILE, JSON.stringify(manifest));
  const anchor = kind === 'query' ? 'useMutation({' : kind === 'form' ? 'receipt);' : 'ready) {';
  const offset = sources.get(file).indexOf(anchor);
  assert.ok(offset >= 0);
  const before = sources.get(file).slice(0, offset);
  const diagnostic = {
    filePath: file.slice('web/app/'.length), plugin: 'react-doctor',
    rule: kind === 'query' ? MUTATION : EVENT,
    message: kind === 'query' ? MUTATION_MESSAGE : EVENT_MESSAGE,
    severity: 'warning', line: before.split('\n').length, column: before.length - before.lastIndexOf('\n'),
  };
  const report = { diagnostics: [diagnostic], summary: { totalDiagnosticCount: 1, suppressedDiagnosticCount: 0 }, error: null };
  const evaluate = () => {
    const raw = structuredClone(report);
    const result = evaluateDiagnosticReviews({ repoRoot, doctorReport: report, toolPackage: TOOL });
    assert.deepEqual(report, raw, 'retain the exact raw rule, position, message, severity and full inventory');
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))), manifest);
    for (const [name, source] of sources) assert.equal(fs.readFileSync(path.join(repoRoot, name), 'utf8'), source);
    return result;
  };
  return { repoRoot, file, kind, sources, targets, declarations, write, evaluate, diagnostic };
}

function assertNative(f) {
  const result = f.evaluate();
  assert.equal(result.status, 'reviewed');
  assert.equal(result.findings.length, 0);
  assert.equal(result.reviewed, 1);
  assert.equal(result.upstreamCorrections.length, 1);
  const proof = result.upstreamCorrections[0];
  assert.deepEqual(proof.diagnostic, f.diagnostic);
  assert.equal(proof.predicate, f.kind === 'query' ? 'mutation-success-invalidation'
    : f.kind === 'form' ? 'external-form-synchronization' : 'query-observation-synchronization');
  assert.equal(proof.provenance.toolPackage, TOOL);
  assert.equal(proof.provenance.sourceCommit, '2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc');
  assert.equal(proof.provenance.ruleSourceSha256, f.kind === 'query'
    ? 'e2803ee846be2117441a2edac81120838a0f62b0e3f306b088b262b3f2ffca9c'
    : '1f89d763b79829a89a3c00e9d655ff8299e26b98c5f7d46a770f3bb0ccf74062');
  assert.equal(proof.provenance.packageIntegrity, 'sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==');
  assert.ok(proof.provenance.references.length > 0);
  for (const evidence of proof.evidence) {
    assert.ok(f.sources.has(evidence.file), 'every resolved source must actually be inspected');
    assert.equal(evidence.sha256, sourceDigest(f.sources.get(evidence.file)));
    assert.ok(evidence.spans.length > 0);
    for (const span of evidence.spans) assert.ok(span.start >= 0 && span.start < span.end && span.end <= f.sources.get(evidence.file).length);
  }
  const owner = proof.evidence.find((entry) => entry.file === f.file);
  assert.ok(owner);
  for (const declaration of f.declarations) {
    assert.ok(owner.spans.some((span) => f.sources.get(f.file).slice(span.start, span.end) === declaration), 'bind the entire actual deferred loader');
  }
  for (const target of f.targets) {
    const evidence = proof.evidence.find((entry) => entry.file === target.file);
    assert.ok(evidence, `missing actual lazy target ${target.file}`);
    assert.ok(evidence.spans.some((span) => f.sources.get(target.file).slice(span.start, span.end) === target.exported), 'bind the selected real component export');
  }
  return proof;
}

function assertHeld(f) {
  const result = f.evaluate();
  assert.equal(result.status, 'requires-review');
  assert.equal(result.reviewed, 0);
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
  assert.equal(result.findings.length, 1);
}

for (const kind of ['query', 'form', 'history']) {
  semanticTest(`${kind} accepts genuine literal deferred component and binds target bytes`, (t) => {
    assertNative(fixture(t, kind));
  });
  semanticTest(`${kind} accepts native default-export import without projection`, (t) => {
    assertNative(fixture(t, kind, { bare: true }));
  });
  semanticTest(`${kind} accepts genuine imported React.lazy facade`, (t) => {
    assertNative(fixture(t, kind, { callee: 'React.lazy' }));
  });
  semanticTest(`${kind} accepts imported and immutable aliases of native lazy`, (t) => {
    assertNative(fixture(t, kind, {
      reactImport: "import React, { lazy as nativeDeferred, useEffect } from 'react';",
      prefix: 'const makeDeferred = nativeDeferred;', callee: 'makeDeferred',
    }));
  });
  semanticTest(`${kind} rejects producer mutation in the deferred target`, (t) => {
    const f = fixture(t, kind);
    f.write(f.targets[0].file, `${kind === 'form'
      ? "import { Form } from 'antd'; Object.assign(Form, { useForm: () => [{ setFieldsValue() {} }] });"
      : "import { QueryClient } from '@tanstack/react-query'; Object.assign(QueryClient.prototype, { invalidateQueries() {} });"}
${f.targets[0].exported}\n`);
    assertHeld(f);
  });
  semanticTest(`${kind} rejects target native producer escape to an opaque port`, (t) => {
    const f = fixture(t, kind);
    f.write(f.targets[0].file, `${kind === 'form'
      ? "import { Form } from 'antd'; patchFactory(Form);"
      : "import { QueryClient } from '@tanstack/react-query'; patchFactory(QueryClient);"}
${f.targets[0].exported}\n`);
    assertHeld(f);
  });
}

for (const kind of ['query', 'history']) {
  semanticTest(`${kind} inspects target-owned same-context receiver as well as factory imports`, (t) => {
    const f = fixture(t, kind);
    f.write(f.targets[0].file, `import { useQueryClient } from '@tanstack/react-query';
export function Panel0() { const cache = useQueryClient(); Object.assign(cache, { invalidateQueries() {} }); return null; }\n`);
    assertHeld(f);
  });
}

for (const [name, options] of [
  ['shadowed local lazy', { reactImport: "import React, { lazy as actualLazy, useEffect } from 'react';", prefix: 'const lazy = foreignLazy;' }],
  ['reassigned lazy alias', { prefix: 'let makeDeferred = lazy; makeDeferred = foreignLazy;', callee: 'makeDeferred' }],
  ['nonzero loader parameter', { loader: "ignored => import('./panel-0').then(module => ({ default: module.Panel0 }))", raw: true }],
  ['patched native React facade', { prefix: 'Object.assign(React, { lazy: foreignLazy });', callee: 'React.lazy' }],
  ['descriptor-patched native React facade', { prefix: "Object.defineProperty(React, 'lazy', { value: foreignLazy });", callee: 'React.lazy' }],
  ['dynamic path', { loader: "import(path).then(module => ({ default: module.Panel0 }))" }],
  ['coerced path', { loader: "import('./' + path).then(module => ({ default: module.Panel0 }))" }],
  ['object path coercion', { loader: "import({ toString() { patchFactory(); return './panel-0'; } }).then(module => ({ default: module.Panel0 }))" }],
  ['captured receiver', { inside: true, loader: "import('./panel-0').then(module => ({ default: (client, module.Panel0) }))" }],
  ['captured native constructor mutation', { loader: "import('./panel-0').then(module => ({ default: (QueryClient.prototype.invalidateQueries = submit, module.Panel0) }))" }],
  ['opaque projection', { loader: "import('./panel-0').then(module => selectComponent(module))" }],
  ['effectful projection statement', { loader: "import('./panel-0').then(module => { submit(); return { default: module.Panel0 }; })" }],
  ['effectful loader statement', { loader: "() => { submit(); return import('./panel-0'); }", raw: true }],
]) {
  semanticTest(`holds uncertified deferred grammar: ${name}`, (t) => {
    const adapted = { ...options };
    if (!adapted.raw) adapted.loader = `() => ${adapted.loader ?? "import('./panel-0').then(module => ({ default: module.Panel0 }))"}`;
    const f = fixture(t, 'history', adapted);
    assertHeld(f);
  });
}

for (const [name, source] of [
  ['dynamic evaluation', 'eval(code); export function Panel0() { return null; }'],
  ['global dynamic constructor', 'globalThis.Function(code)(); export function Panel0() { return null; }'],
  ['nested dynamic import', "export function Panel0() { const load = () => import(path); return null; }"],
  ['eager dynamic import', "const eager = import('./secondary'); export function Panel0() { return null; }"],
  ['mutable selected export', 'export let Panel0 = () => null; Panel0 = foreignComponent;'],
  ['missing selected export', 'export function AnotherPanel() { return null; }'],
]) {
  semanticTest(`holds uncertified target source: ${name}`, (t) => {
    const f = fixture(t);
    f.write(f.targets[0].file, `${source}\n`);
    f.write('web/app/src/secondary.tsx', 'export default function Secondary() { return null; }\n');
    assertHeld(f);
  });
}

semanticTest('holds a side-effect-only executable import in the target until its producer is proven', (t) => {
  const f = fixture(t);
  f.write(f.targets[0].file, "import './mutating'; export function Panel0() { return null; }\n");
  f.write('web/app/src/mutating.ts', "import { QueryClient } from '@tanstack/react-query'; Object.assign(QueryClient.prototype, { invalidateQueries() {} });\n");
  assertHeld(f);
});

semanticTest('holds missing literal target source', (t) => {
  const f = fixture(t);
  fs.unlinkSync(path.join(f.repoRoot, f.targets[0].file));
  f.sources.delete(f.targets[0].file);
  assertHeld(f);
});

semanticTest('holds a root eager import outside any native deferred loader', (t) => {
  const f = fixture(t, 'history', { prefix: "const eager = import('./panel-0');" });
  assertHeld(f);
});

semanticTest('holds a nested dynamic loader outside the certified native lazy grammar', (t) => {
  const f = fixture(t, 'history', { prefix: "const unknownLoader = () => import('./panel-0');" });
  assertHeld(f);
});

semanticTest('keeps foreign same-named local objects separate from the native lazy producer', (t) => {
  const f = fixture(t, 'history', { prefix: 'const foreign = { lazy: () => null }; foreign.lazy();' });
  f.write(f.targets[0].file, `const QueryClient = { prototype: {} }; Object.assign(QueryClient.prototype, { invalidateQueries() {} });
${f.targets[0].exported}\n`);
  assertNative(f);
});

semanticTest('accepts exactly six total observed code units including key helper and four targets', (t) => {
  const f = fixture(t, 'history', { count: 4 });
  const proof = assertNative(f);
  assert.equal(proof.evidence.length, 6);
  assert.deepEqual(new Set(proof.evidence.map((entry) => entry.file)),
    new Set([f.file, 'web/app/src/records-key.ts', ...f.targets.map((entry) => entry.file)]));
});

semanticTest('holds seven total units instead of resetting the module budget for deferred targets', (t) => {
  assertHeld(fixture(t, 'history', { count: 5 }));
});

// CSS is data under the current Vite contract. These are not JS/TS code units.
// Its installed PostCSS parser must validate bytes without loading user plugins.
function styles(f, entries) {
  const target = f.targets[0];
  const imports = entries.map(([name, source]) => {
    const file = `web/app/src/${name}`;
    if (source !== null) f.write(file, source);
    return { file, declaration: `import './${name}';` };
  });
  f.write(target.file, `${imports.map(({ declaration }) => declaration).join('\n')}\n${target.exported}\n`);
  return imports;
}

function assertStyles(f, imports) {
  const proof = assertNative(f);
  const targetSource = f.sources.get(f.targets[0].file);
  const importer = proof.evidence.find((entry) => entry.file === f.targets[0].file);
  for (const { file, declaration } of imports) {
    const asset = proof.evidence.find((entry) => entry.file === file);
    assert.ok(asset, `missing inspected stylesheet ${file}`);
    assert.equal(asset.sha256, sourceDigest(f.sources.get(file)));
    assert.ok(asset.spans.some(({ start, end }) => start === 0 && end === f.sources.get(file).length));
    const start = targetSource.indexOf(declaration);
    assert.ok(importer.spans.some((span) => span.start === start && span.end === start + declaration.length), 'bind each stylesheet import to its actual importer source');
  }
  return proof;
}

for (const kind of ['query', 'form', 'history']) {
  semanticTest(`${kind} accepts a parsed literal relative CSS asset with digest and import span`, (t) => {
    const f = fixture(t, kind);
    assertStyles(f, styles(f, [['panel.css', '.panel { display: none; }\n']]));
  });
}

for (const [name, asset, source] of [
  ['missing stylesheet', 'missing.css', null],
  ['query stylesheet mode', 'panel.css?inline', '.panel { display: none; }'],
  ['hash stylesheet mode', 'panel.css#fragment', '.panel { display: none; }'],
  ['unknown stylesheet extension', 'panel.scss', '.panel { display: none; }'],
  ['JS source masquerading as CSS', 'panel.css', "import { QueryClient } from '@tanstack/react-query'; Object.assign(QueryClient.prototype, { invalidateQueries() {} });"],
  ['JS suffix masquerading as stylesheet', 'panel.css.js', "import { QueryClient } from '@tanstack/react-query'; Object.assign(QueryClient.prototype, { invalidateQueries() {} });"],
  ['malformed CSS source', 'panel.css', '.panel { color:'],
]) {
  semanticTest(`holds uncertified asset: ${name}`, (t) => {
    const f = fixture(t);
    styles(f, [[asset, source]]);
    assertHeld(f);
  });
}

semanticTest('holds a stylesheet symlink whose canonical target leaves the repository', (t) => {
  const f = fixture(t);
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'doctor-outside-css-'));
  t.after(() => fs.rmSync(outside, { recursive: true, force: true }));
  const source = '.panel { display: none; }\n';
  fs.writeFileSync(path.join(outside, 'outside.css'), source);
  const [{ file }] = styles(f, [['panel.css', source]]);
  fs.unlinkSync(path.join(f.repoRoot, file));
  fs.symlinkSync(path.join(outside, 'outside.css'), path.join(f.repoRoot, file));
  assertHeld(f);
});

semanticTest('holds non-UTF8 stylesheet bytes instead of digesting replacement characters', (t) => {
  const f = fixture(t);
  const [{ file }] = styles(f, [['panel.css', null]]);
  fs.writeFileSync(path.join(f.repoRoot, file), Buffer.from([0xff, 0xfe, 0x7b, 0x7d]));
  assertHeld(f);
});

semanticTest('CSS data does not consume any of the six total JS/TS code units', (t) => {
  const f = fixture(t, 'history', { count: 4 });
  const imports = styles(f, [['panel.css', '.panel { display: none; }\n']]);
  const proof = assertStyles(f, imports);
  const assetPaths = new Set(imports.map(({ file }) => file));
  assert.equal(proof.evidence.filter((entry) => !assetPaths.has(entry.file)).length, 6);
  assert.equal(proof.evidence.filter((entry) => assetPaths.has(entry.file)).length, 1);
});

const cssBytes = (bytes) => `/*${'x'.repeat(bytes - 4)}*/`;

semanticTest('accepts the 64 KiB UTF8 byte limit for one stylesheet', (t) => {
  const f = fixture(t);
  assertStyles(f, styles(f, [['panel.css', cssBytes(64 * 1024)]]));
});

semanticTest('holds one stylesheet beyond the 64 KiB UTF8 byte limit', (t) => {
  const f = fixture(t);
  styles(f, [['panel.css', cssBytes(64 * 1024 + 1)]]);
  assertHeld(f);
});

semanticTest('measures stylesheet byte limits in UTF8 bytes rather than code units', (t) => {
  const f = fixture(t);
  styles(f, [['panel.css', `/*${'界'.repeat(22 * 1024)}*/`]]);
  assertHeld(f);
});

semanticTest('accepts four unique assets at the shared 128 KiB total limit', (t) => {
  const f = fixture(t);
  assertStyles(f, styles(f, Array.from({ length: 4 }, (_, index) => [`panel-${index}.css`, cssBytes(32 * 1024)])));
});

semanticTest('holds a fifth unique asset instead of resetting the asset count per import', (t) => {
  const f = fixture(t);
  styles(f, Array.from({ length: 5 }, (_, index) => [`panel-${index}.css`, '.panel {}']));
  assertHeld(f);
});

semanticTest('holds total asset bytes beyond 128 KiB even when every asset is below its own cap', (t) => {
  const f = fixture(t);
  styles(f, Array.from({ length: 4 }, (_, index) => [`panel-${index}.css`, cssBytes(32 * 1024 + 1)]));
  assertHeld(f);
});

semanticTest('genuine CSS comments and strings may contain JavaScript-looking text', (t) => {
  const f = fixture(t);
  assertStyles(f, styles(f, [['panel.css', `/* import { QueryClient } from '@tanstack/react-query'; eval(code); */
.panel::before { content: "Object.assign(QueryClient.prototype, { invalidateQueries() {} });"; }
`]]));
});

semanticTest('repeated canonical asset imports share bytes while retaining every importer span', (t) => {
  const f = fixture(t, 'history', { count: 3 });
  const imports = styles(f, [['panel.css', cssBytes(64 * 1024)]]);
  for (const target of f.targets.slice(1)) f.write(target.file, `import './panel.css';\n${target.exported}\n`);
  const proof = assertStyles(f, imports);
  assert.equal(proof.evidence.filter((entry) => entry.file === imports[0].file).length, 1, 'three imports must not become three asset byte allocations');
  for (const target of f.targets) {
    const evidence = proof.evidence.find((entry) => entry.file === target.file);
    assert.ok(evidence.spans.some(({ start, end }) => start === 0 && end === "import './panel.css';".length), 'each actual importer retains its own declaration span');
  }
});

semanticTest('stylesheet parse walks consume the existing shared 20k proof step budget', (t) => {
  const f = fixture(t);
  styles(f, [['first.css', 'a{}'.repeat(10001)], ['second.css', 'a{}'.repeat(10001)]]);
  assertHeld(f);
});
