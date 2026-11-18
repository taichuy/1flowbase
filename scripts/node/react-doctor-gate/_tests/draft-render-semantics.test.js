const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createRequire } = require('node:module');
const { REVIEW_FILE, sourceDigest, evaluateDiagnosticReviews } = require('../diagnostic-review.js');

const ROOT = path.resolve(__dirname, '../../../..');
const TOOL = 'react-doctor@0.2.16';
const COMMIT = '2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc';
const INTEGRITY = 'sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==';
const DRAFT = 'no-derived-useState';
const RENDER = 'rerender-state-only-in-handlers';
const RULES = {
  [DRAFT]: ['cc7730387e948eb92f3bc614156bdc57a9a5ddf50887a74f33718cc0e3c77991', 'state-and-effects/no-derived-use-state.ts'],
  [RENDER]: ['92f15323842cd00786ad17fe24178c448b57c584fd760cfb9207b64f21eff98a', 'state-and-effects/rerender-state-only-in-handlers.ts'],
};
const semanticTest = (name, fn) => test(`draft render semantic correction: ${name}`, fn);
const draftMessage = (name) => `Your users see a stale value when prop "${name}" changes because useState copies it once.`;
const renderMessage = (name) => `Each update to "${name}" redraws your component for nothing because this useState is set but never shown on screen.`;

function fixture(t, source, file = 'web/app/src/renamed-editor/owner.tsx') {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'doctor-draft-proof-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const sources = new Map();
  const write = (name, body) => {
    fs.mkdirSync(path.dirname(path.join(repoRoot, name)), { recursive: true });
    fs.writeFileSync(path.join(repoRoot, name), body);
    sources.set(name, body);
  };
  write(file, source);
  write('web/app/package.json', JSON.stringify({ dependencies: { react: '19.2.5', antd: '6.6.2' } }));
  const manifest = { schemaVersion: 1, toolPackage: TOOL, reviews: [] };
  write(REVIEW_FILE, JSON.stringify(manifest));
  const doctorReport = { diagnostics: [], summary: { totalDiagnosticCount: 0, suppressedDiagnosticCount: 0 }, error: null };
  const add = (rule, anchor, name, filePath = file, occurrence = 0) => {
    const source = sources.get(filePath);
    let offset = -1;
    for (let index = 0; index <= occurrence; index++) offset = source.indexOf(anchor, offset + 1);
    assert.ok(offset >= 0, `missing diagnostic anchor ${anchor} in ${filePath}`);
    const before = source.slice(0, offset);
    const diagnostic = { filePath: filePath.slice('web/app/'.length), plugin: 'react-doctor', rule,
      severity: 'warning', message: rule === DRAFT ? draftMessage(name) : renderMessage(name),
      line: before.split('\n').length, column: before.length - before.lastIndexOf('\n') };
    doctorReport.diagnostics.push(diagnostic);
    doctorReport.summary.totalDiagnosticCount++;
    return diagnostic;
  };
  const evaluate = (toolPackage = TOOL) => {
    const raw = structuredClone(doctorReport);
    const result = evaluateDiagnosticReviews({ repoRoot, doctorReport, toolPackage });
    assert.deepEqual(doctorReport, raw, 'retain complete raw diagnostics, counts, locations and severity');
    for (const [file, source] of sources) assert.equal(fs.readFileSync(path.join(repoRoot, file), 'utf8'), source);
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))), manifest,
      'no new file, component, prop, key or rule review exception');
    return result;
  };
  return { repoRoot, file, sources, write, doctorReport, add, evaluate };
}
function assertProof(f, result, diagnostics, predicate, snippets = []) {
  assert.equal(result.status, 'reviewed');
  assert.equal(result.findings.length, 0);
  assert.equal(result.upstreamCorrections.length, diagnostics.length);
  for (const diagnostic of diagnostics) {
    const proof = result.upstreamCorrections.find((entry) => JSON.stringify(entry.diagnostic) === JSON.stringify(diagnostic));
    assert.ok(proof);
    assert.equal(proof.predicate, predicate);
    assert.equal(proof.provenance.toolPackage, TOOL);
    assert.equal(proof.provenance.packageIntegrity, INTEGRITY);
    assert.equal(proof.provenance.sourceCommit, COMMIT);
    assert.equal(proof.provenance.ruleSourceSha256, RULES[diagnostic.rule][0]);
    assert.ok(proof.provenance.references.includes(`https://github.com/millionco/react-doctor/blob/${COMMIT}/packages/oxlint-plugin-react-doctor/src/plugin/rules/${RULES[diagnostic.rule][1]}`));
    assert.ok(proof.reason.length > 20);
    assert.ok(Array.isArray(proof.evidence) && proof.evidence.length > 0);
    for (const evidence of proof.evidence) {
      const source = f.sources.get(evidence.file);
      assert.equal(typeof source, 'string');
      assert.equal(evidence.sha256, sourceDigest(source));
      assert.ok(evidence.spans.length > 0);
      for (const span of evidence.spans) assert.ok(Number.isInteger(span.start) && Number.isInteger(span.end)
        && span.start >= 0 && span.start < span.end && span.end <= source.length);
    }
    for (const snippet of snippets) assert.ok(proof.evidence.some((evidence) => evidence.spans.some((span) =>
      f.sources.get(evidence.file).slice(span.start, span.end).includes(snippet))), `missing owner/consumer proof: ${snippet}`);
  }
}
function unresolved(f, diagnostic, toolPackage = TOOL) {
  const result = f.evaluate(toolPackage);
  assert.equal(result.status, 'requires-review');
  assert.equal(result.findings.length, 1);
  assert.equal(result.findings[0].rule, `${diagnostic.plugin}/${diagnostic.rule}`);
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
}

const fields = `<input aria-label="Title" value={draft.title} onChange={(event) => setDraft(previous => ({ ...previous, title: event.target.value }))} />`;
const actions = `<button onClick={close}>Cancel</button><button onClick={async () => { if (await onCommit(draft)) close(); }}>Apply</button>`;
const modalSource = `import { useState } from 'react';
type Document = { title: string; description?: string };
type EditorProps = { source: Document; onCommit(value: Document): Promise<boolean>; close(): void };
function Editor({ source, onCommit, close }: EditorProps) {
  const [draft, setDraft] = useState(source);
  return <section role="dialog">${fields}${actions}</section>;
}
export function Host({ source, ownerId, onCommit }: { source: Document; ownerId: 'a' | 'b'; onCommit(value: Document): Promise<boolean> }) {
  const [open, setOpen] = useState(false);
  return <><button onClick={() => setOpen(true)}>Open</button>{open && <Editor key={ownerId} source={source} onCommit={onCommit} close={() => setOpen(false)} />}</>;
}`;
const persistentSource = `import { useState } from 'react';
export function Panel({ source, onCommit }) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(source);
  const begin = () => { setDraft(source); setOpen(true); };
  const close = () => setOpen(false);
  return <><button onClick={begin}>Open</button>{open && <section role="dialog">${fields}${actions}</section>}</>;
}`;

semanticTest('proves a private editable snapshot with apply/cancel and real conditional mount ownership', (t) => {
  const f = fixture(t, modalSource);
  const diagnostic = f.add(DRAFT, 'useState(source)', 'source');
  assertProof(f, f.evaluate(), [diagnostic], 'editable-session-draft', ['useState(source)', 'onCommit(draft)', 'open && <Editor', '...previous']);
});
semanticTest('proves explicit new-open snapshot reset for a persistent editor', (t) => {
  const f = fixture(t, persistentSource);
  const diagnostic = f.add(DRAFT, 'useState(source)', 'source');
  assertProof(f, f.evaluate(), [diagnostic], 'editable-session-draft', ['setDraft(source)', 'setOpen(true)', 'onCommit(draft)']);
});
semanticTest('resolves imported controlled fields and a declared private props alias', (t) => {
  const f = fixture(t, modalSource.replace("import { useState } from 'react';", "import { useState } from 'react'; import { Fields } from './fields';")
    .replace(fields, '<Fields value={draft} onEdit={(title) => setDraft(previous => ({ ...previous, title }))} />'));
  const fieldFile = 'web/app/src/renamed-editor/fields.tsx';
  f.write(fieldFile, `type Props = { value: { title: string }; onEdit(title: string): void };
    export function Fields({ value, onEdit }: Props) { return <input value={value.title} onChange={(event) => onEdit(event.target.value)} />; }`);
  const diagnostic = f.add(DRAFT, 'useState(source)', 'source');
  assertProof(f, f.evaluate(), [diagnostic], 'editable-session-draft', ['onEdit(event.target.value)', 'value={draft}']);
});

for (const [name, source] of [
  ['pure copied display', `import { useState } from 'react'; export function Editor({ source }) { const [draft] = useState(source); return <p>{draft.title}</p>; }`],
  ['only an unrelated event update', `import { useState } from 'react'; export function Editor({ source }) { const [draft] = useState(source); const [count, setCount] = useState(0); return <><p>{draft.title}</p><button onClick={() => setCount(count + 1)}>Count</button></>; }`],
  ['continuous effect mirroring', modalSource.replace("import { useState }", "import { useState, useEffect }").replace('return <section role="dialog">', 'useEffect(() => setDraft(source), [source]); return <section role="dialog">')],
  ['render-time copy', modalSource.replace('return <section role="dialog">', 'if (draft !== source) setDraft(source); return <section role="dialog">')],
  ['applies source instead of edited state', modalSource.replace('onCommit(draft)', 'onCommit(source)')],
  ['cancellation commits edits', modalSource.replace('<button onClick={close}>Cancel</button>', '<button onClick={() => { onCommit(draft); close(); }}>Cancel</button>')],
  ['setter escapes to unknown owner', modalSource.replace('return <section role="dialog">', 'register(setDraft); return <section role="dialog">')],
  ['stale opener closure', persistentSource.replace("import { useState }", "import { useState, useCallback }").replace('const begin = () => { setDraft(source); setOpen(true); };', 'const begin = useCallback(() => { setDraft(source); setOpen(true); }, []);')],
  ['deferred reset after dialog opens', persistentSource.replace('const begin = () => { setDraft(source); setOpen(true); };', 'const begin = async () => { setOpen(true); await load(); setDraft(source); };')],
  ['persistent owner has no new-open reset', persistentSource.replace('setDraft(source); setOpen(true);', 'setOpen(true);')],
  ['prop spread overwrites previous edited fields', modalSource.replace('...previous, title:', '...previous, ...source, title:')],
  ['unknown controlled widget contract', modalSource.replace(fields, '<Unknown value={draft} onChange={setDraft} />')],
  ['key alone with no editing transaction', `import { useState } from 'react'; function Editor({ source }) { const [draft] = useState(source); return <p>{draft.title}</p>; } export function Host({ source }) { return <Editor key={source.id} source={source} />; }`],
]) {
  semanticTest(`keeps copied-prop or unproven session warning: ${name}`, (t) => {
    const f = fixture(t, source);
    unresolved(f, f.add(DRAFT, 'useState(source)', 'source'));
  });
}

semanticTest('projects all four actual initializer findings with controlled-field and reset-owner source evidence', (t) => {
  const prefix = 'web/app/src/features/frontstage/';
  const form = prefix + 'components/data-blocks/form/FormDesignControls.tsx';
  const table = prefix + 'components/data-blocks/table/TableSettings.tsx';
  const files = [form, table,
    prefix + 'components/data-blocks/form/design-settings/FormItemSettingsFields.tsx',
    prefix + 'components/data-blocks/form/design-settings/FormLayoutSettingsFields.tsx',
    prefix + 'components/data-blocks/form/design-settings/FormRuleSettingsFields.tsx',
    prefix + 'lib/block-settings/registry-store.ts',
    prefix + 'lib/native-modules/block-design/runtime.tsx',
    prefix + 'components/data-blocks/shared/DataBlockModal.tsx',
    prefix + 'components/data-blocks/shared/DataRelationEditor.tsx',
    prefix + 'components/data-blocks/shared/DataBlockFilterEditor.tsx',
    prefix + 'components/data-blocks/shared/filter-editor/FilterConditionRow.tsx',
    prefix + 'components/data-blocks/form/FormFieldControl.tsx',
    prefix + 'lib/data-blocks/types.ts'];
  const f = fixture(t, fs.readFileSync(path.join(ROOT, form), 'utf8'), form);
  for (const file of files) f.write(file, fs.readFileSync(path.join(ROOT, file), 'utf8'));
  const diagnostics = [f.add(DRAFT, 'useState(config)', 'config', form),
    f.add(DRAFT, 'useState(config)', 'config', table), f.add(DRAFT, 'useState(column)', 'column', table),
    f.add(DRAFT, 'useState(action)', 'action', table)];
  assertProof(f, f.evaluate(), diagnostics, 'editable-session-draft');
});

const collectionSource = `import { useState } from 'react';
type Row = { id: string; title: string };
function Menu({ items }) { return <ul>{items.map(item => <li key={item.key}>{item.label}</li>)}</ul>; }
export function Choice({ rows }: { rows: readonly Row[] }) {
  const [search, setSearch] = useState('');
  const items = [];
  items.push({ key: 'search', label: <input aria-label="Search" value={search} onChange={(event) => setSearch(event.target.value)} /> });
  items.push(...rows.filter(row => row.title.toLowerCase().includes(search.toLowerCase())).map(row => ({ key: row.id, label: <span>{row.title}</span> })));
  return <Menu items={items} />;
}`;
semanticTest('follows eager array population to an actually rendered controlled search and result list', (t) => {
  const f = fixture(t, collectionSource);
  const diagnostic = f.add(RENDER, '[search, setSearch]', 'search');
  assertProof(f, f.evaluate(), [diagnostic], 'rendered-collection-state', ['items.push', 'value={search}', '<Menu items={items}', 'item.label']);
});
for (const [name, source] of [
  ['populated array never consumed', collectionSource.replace('return <Menu items={items} />;', 'return <button>Open</button>;')],
  ['render receives unrelated array', collectionSource.replace('<Menu items={items}', '<Menu items={[]}')],
  ['collection only populated in an event', collectionSource.replace('  items.push({', '  const prepare = () => { items.push({').replace('  return <Menu', '  }; return <Menu')],
  ['state only used by a deferred item handler', collectionSource.replace('label: <input aria-label="Search" value={search} onChange={(event) => setSearch(event.target.value)} />', 'label: <button onClick={() => consume(search)}>Use</button>')
    .replace('rows.filter(row => row.title.toLowerCase().includes(search.toLowerCase()))', 'rows')],
  ['opaque receiving menu', collectionSource.replace('function Menu({ items }) { return <ul>{items.map(item => <li key={item.key}>{item.label}</li>)}</ul>; }', "import { Menu } from 'unknown';")],
  ['known consumer discards collection', collectionSource.replace('return <ul>{items.map(item => <li key={item.key}>{item.label}</li>)}</ul>;', 'return <ul />;')],
  ['unknown map schedules callbacks', collectionSource.replace('rows: readonly Row[]', 'rows: any')
    .replace('const items = [];', 'const items = []; rows = makeDeferredIterator(rows);')
    .replace('label: <input aria-label="Search" value={search} onChange={(event) => setSearch(event.target.value)} />', 'label: <button onClick={() => setSearch("needle")}>Find</button>')],
  ['reassigned populated array', collectionSource.replace('const items = [];', 'let items = [];').replace('return <Menu', 'items = []; return <Menu')],
  ['shadowed state binding', `import { useState } from 'react'; export function Choice() { const [search, setSearch] = useState(''); const items = []; { const search = 'constant'; items.push({ key: 'x', label: <span>{search}</span> }); } return <><button onClick={() => setSearch('event-only')}>Set</button><ul>{items.map(item => <li>{item.label}</li>)}</ul></>; }`],
]) {
  semanticTest(`does not invent a render dependency: ${name}`, (t) => {
    const f = fixture(t, source);
    unresolved(f, f.add(RENDER, '[search, setSearch]', 'search'));
  });
}
semanticTest('projects actual CreateDataBlockMenu search through the rendered AntD menu contract', (t) => {
  const file = 'web/app/src/features/frontstage/components/data-blocks/CreateDataBlockMenu.tsx';
  const f = fixture(t, fs.readFileSync(path.join(ROOT, file), 'utf8'), file);
  const diagnostic = f.add(RENDER, '[search, setSearch]', 'search');
  assertProof(f, f.evaluate(), [diagnostic], 'rendered-collection-state', ['value={search}', 'items.push', 'menu={']);
});
semanticTest('requires current receiver/source evidence and the exact pinned diagnostic identity', (t) => {
  const f = fixture(t, modalSource);
  const diagnostic = f.add(DRAFT, 'useState(source)', 'source');
  const original = { ...diagnostic };
  for (const changes of [{ message: draftMessage('unrelated') }, { plugin: 'other' }, { line: 9999 }, { column: 9999 }]) {
    Object.assign(diagnostic, original, changes);
    unresolved(f, diagnostic);
  }
  Object.assign(diagnostic, original);
  unresolved(f, diagnostic, 'react-doctor@0.2.17');
  f.write(f.file, 'export const invalid = (');
  unresolved(f, diagnostic);
});

function compile(source) {
  const fromWeb = createRequire(path.join(ROOT, 'web/package.json'));
  const fromApp = createRequire(path.join(ROOT, 'web/app/package.json'));
  const ts = fromWeb('typescript');
  const text = ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX,
  } }).outputText;
  const module = { exports: {} };
  new Function('require', 'module', 'exports', text)(fromApp, module, module.exports);
  return module.exports;
}
async function browser(t) {
  const fromWeb = createRequire(path.join(ROOT, 'web/package.json'));
  const fromApp = createRequire(path.join(ROOT, 'web/app/package.json'));
  const { JSDOM } = fromWeb('jsdom');
  const dom = new JSDOM('<!doctype html><html><body><div id="mount"></div></body></html>', { url: 'http://localhost/' });
  const values = { window: dom.window, document: dom.window.document, navigator: dom.window.navigator,
    HTMLElement: dom.window.HTMLElement, Element: dom.window.Element, Node: dom.window.Node, IS_REACT_ACT_ENVIRONMENT: true };
  const previous = new Map(Object.keys(values).map((key) => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
  for (const [key, value] of Object.entries(values)) Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
  const React = fromApp('react');
  const root = fromApp('react-dom/client').createRoot(dom.window.document.getElementById('mount'));
  t.after(async () => {
    try { await React.act(async () => root.unmount()); }
    finally {
      dom.window.close();
      for (const [key, descriptor] of previous) {
        if (descriptor) Object.defineProperty(globalThis, key, descriptor);
        else delete globalThis[key];
      }
    }
  });
  const click = (name) => React.act(async () => {
    const button = [...dom.window.document.querySelectorAll('button')].find((node) => node.textContent === name);
    assert.ok(button, `missing action ${name}`);
    button.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
  });
  const input = (name, value) => React.act(async () => {
    const node = dom.window.document.querySelector(`input[aria-label="${name}"]`);
    Object.getOwnPropertyDescriptor(dom.window.HTMLInputElement.prototype, 'value').set.call(node, value);
    node.dispatchEvent(new dom.window.Event('input', { bubbles: true }));
  });
  return { React, dom, click, input, render: (element) => React.act(async () => root.render(element)) };
}
for (const [name, source, exportName] of [['conditional owner', modalSource, 'Host'], ['explicit open reset', persistentSource, 'Panel']]) {
  semanticTest(`runtime: ${name} retains same-session edits, cancels without save, reopens latest source`, async (t) => {
    const view = await browser(t);
    const Component = compile(source)[exportName];
    const committed = [];
    const onCommit = async (value) => { committed.push(value); return false; };
    await view.render(view.React.createElement(Component, { source: { title: 'Original' }, ownerId: 'a', onCommit }));
    await view.click('Open');
    await view.input('Title', 'Edited draft');
    await view.render(view.React.createElement(Component, { source: { title: 'Latest source' }, ownerId: 'a', onCommit }));
    assert.equal(view.dom.window.document.querySelector('input').value, 'Edited draft');
    await view.click('Apply');
    assert.deepEqual(committed, [{ title: 'Edited draft' }]);
    assert.ok(view.dom.window.document.querySelector('[role="dialog"]'), 'failed save retains editing session');
    await view.click('Cancel');
    assert.equal(committed.length, 1, 'cancel never saves');
    await view.click('Open');
    assert.equal(view.dom.window.document.querySelector('input').value, 'Latest source');
  });
}
semanticTest('runtime: rendered collection search changes visible records without selecting anything', async (t) => {
  const view = await browser(t);
  const { Choice } = compile(collectionSource);
  await view.render(view.React.createElement(Choice, { rows: [{ id: '1', title: 'Alpha' }, { id: '2', title: 'Beta' }] }));
  assert.match(view.dom.window.document.body.textContent, /Alpha/u);
  await view.input('Search', 'beta');
  assert.doesNotMatch(view.dom.window.document.body.textContent, /Alpha/u);
  assert.match(view.dom.window.document.body.textContent, /Beta/u);
});
