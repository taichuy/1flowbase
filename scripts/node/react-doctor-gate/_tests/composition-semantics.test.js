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
const SLOT = 'jsx-no-jsx-as-prop';
const RENDER = 'no-render-in-render';
const CHILDREN = 'no-polymorphic-children';
const REF = 'no-react19-deprecated-apis';
const RULES = {
  [SLOT]: ['6c7d5e6da3dd051357f524e31491c08be6ca0be8d128a04564c796d53fcfd24c', 'react-builtins/jsx-no-jsx-as-prop.ts'],
  [RENDER]: ['9bcda8ae0e0342fdc72780a91481dc8bc757f3e6e5a7e632ad9e6d1a57558720', 'architecture/no-render-in-render.ts'],
  [CHILDREN]: ['986bc09f88e535b9eecffe17b2ca8a5fb118c08cd8d96ea1ff6bc4c1375913ca', 'correctness/no-polymorphic-children.ts'],
  [REF]: ['fcb5ff5c9c0859840f869f9afc0550fd9014358732fe67b81c2435b5501dbca2', 'architecture/no-react-dom-deprecated-apis.ts'],
};
const MESSAGES = {
  [SLOT]: 'This child redraws every render because the prop gets brand new JSX each time.',
  [RENDER]: 'Your users lose state because "renderEntry()" builds UI from an inline call that React remounts, so pull it into its own component instead.',
  [CHILDREN]: 'Your users hit inconsistent behavior because `typeof children === "string"` makes this component switch on what callers pass, so add clear subcomponents like `<Button.Text>` instead.',
  [REF]: 'forwardRef is dead weight in React 19, since ref is a normal prop now, so drop it & pass ref straight through.',
};
const semanticTest = (name, run) => test(`composition semantic correction: ${name}`, run);

function fixture(t, source, file = 'web/app/src/renamed-feature/owner.tsx') {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'doctor-composition-proof-'));
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
  const doctorReport = {
    diagnostics: [], summary: { totalDiagnosticCount: 0, suppressedDiagnosticCount: 0 }, error: null,
  };
  const add = (rule, anchor, target = file, occurrence = 0) => {
    const source = sources.get(target);
    let offset = -1;
    for (let index = 0; index <= occurrence; index++) offset = source.indexOf(anchor, offset + 1);
    assert.ok(offset >= 0, `missing ${anchor} in ${target}`);
    const before = source.slice(0, offset);
    const diagnostic = {
      filePath: target.slice('web/app/'.length), plugin: 'react-doctor', rule, severity: 'warning',
      message: MESSAGES[rule], line: before.split('\n').length, column: before.length - before.lastIndexOf('\n'),
    };
    doctorReport.diagnostics.push(diagnostic);
    doctorReport.summary.totalDiagnosticCount++;
    return diagnostic;
  };
  const evaluate = (toolPackage = TOOL) => {
    const raw = structuredClone(doctorReport);
    const result = evaluateDiagnosticReviews({ repoRoot, doctorReport, toolPackage });
    assert.deepEqual(doctorReport, raw, 'preserve raw diagnostic positions, messages, severities and inventory');
    for (const [file, source] of sources) {
      assert.equal(fs.readFileSync(path.join(repoRoot, file), 'utf8'), source, 'proof must not modify sources or review manifest');
    }
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))), manifest);
    return result;
  };
  return { repoRoot, file, sources, write, doctorReport, add, evaluate };
}

function assertProof(f, result, diagnostic, predicate, snippets = []) {
  assert.equal(result.status, 'reviewed');
  assert.equal(result.findings.length, 0);
  const proof = result.upstreamCorrections.find((item) => JSON.stringify(item.diagnostic) === JSON.stringify(diagnostic));
  assert.ok(proof);
  assert.equal(proof.predicate, predicate);
  assert.equal(proof.provenance.toolPackage, TOOL);
  assert.equal(proof.provenance.packageIntegrity, INTEGRITY);
  assert.equal(proof.provenance.sourceCommit, COMMIT);
  assert.equal(proof.provenance.ruleSourceSha256, RULES[diagnostic.rule][0]);
  const upstream = `https://github.com/millionco/react-doctor/blob/${COMMIT}/packages/oxlint-plugin-react-doctor/src/plugin/rules/${RULES[diagnostic.rule][1]}`;
  assert.ok(proof.provenance.references.includes(upstream));
  assert.ok(proof.reason.length > 20);
  assert.ok(Array.isArray(proof.evidence) && proof.evidence.length > 0);
  for (const evidence of proof.evidence) {
    const source = f.sources.get(evidence.file);
    assert.equal(typeof source, 'string');
    assert.equal(evidence.sha256, sourceDigest(source));
    assert.ok(evidence.spans.length > 0);
    for (const span of evidence.spans) {
      assert.ok(Number.isInteger(span.start) && Number.isInteger(span.end));
      assert.ok(span.start >= 0 && span.start < span.end && span.end <= source.length);
    }
  }
  for (const text of snippets) {
    assert.ok(proof.evidence.some((evidence) => evidence.spans.some((span) =>
      f.sources.get(evidence.file).slice(span.start, span.end).includes(text))), `missing proof source: ${text}`);
  }
  return proof;
}
function assertUnresolved(f, diagnostic, toolPackage = TOOL) {
  const result = f.evaluate(toolPackage);
  assert.equal(result.status, 'requires-review');
  assert.equal(result.findings.length, 1);
  assert.equal(result.findings[0].rule, `${diagnostic.plugin}/${diagnostic.rule}`);
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
}

const slotOwner = `import { Surface as ReceivingView } from './receiver';
export function Caller({ count }) { return <ReceivingView payload={<input value={count} readOnly />} />; }`;
const receiverFile = 'web/app/src/renamed-feature/receiver.tsx';

for (const [name, receiver] of [
  ['ordinary function', 'export function Surface({ payload }) { return <section>{payload}</section>; }'],
  ['ordinary const alias', 'const Body = ({ payload }) => <section>{payload}</section>; export const Surface = Body;'],
  ['genuine forwardRef', `import { forwardRef as expose } from 'react'; export const Surface = expose(function Body({ payload }, ref) { return <section ref={ref}>{payload}</section>; });`],
]) {
  semanticTest(`resolves a renamed imported nonmemoized slot receiver: ${name}`, (t) => {
    const f = fixture(t, slotOwner);
    f.write(receiverFile, receiver);
    const diagnostic = f.add(SLOT, 'payload={');
    const proof = assertProof(f, f.evaluate(), diagnostic, 'nonmemoized-slot-receiver', ['payload={', '{payload}']);
    assert.ok(proof.evidence.some((entry) => entry.file === receiverFile), 'receiver source must be part of the proof');
  });
}

for (const [name, receiver] of [
  ['memoized receiver', `import { memo } from 'react'; export const Surface = memo(({ payload }) => <section>{payload}</section>);`],
  ['memoized forwardRef', `import { memo, forwardRef } from 'react'; export const Surface = memo(forwardRef(({ payload }, ref) => <section ref={ref}>{payload}</section>));`],
  ['unknown HOC', `import { wrap } from 'unknown'; export const Surface = wrap(({ payload }) => <section>{payload}</section>);`],
  ['local forwardRef spelling', 'const forwardRef = (render) => render; export const Surface = forwardRef(({ payload }) => <section>{payload}</section>);'],
  ['lazy receiver', `import { lazy } from 'react'; export const Surface = lazy(() => import('./actual'));`],
  ['class boundary', `import { PureComponent } from 'react'; export class Surface extends PureComponent { render() { return <section>{this.props.payload}</section>; } }`],
  ['mutated exported binding', 'export let Surface = ({ payload }) => <section>{payload}</section>; Surface = wrap(Surface);'],
  ['unknown re-export', `export { Surface } from 'unknown';`],
  ['effect consumes slot identity', `import { useEffect } from 'react'; export function Surface({ payload }) { useEffect(() => consume(payload), [payload]); return <section>{payload}</section>; }`],
]) {
  semanticTest(`does not turn an unproven receiver into a performance exemption: ${name}`, (t) => {
    const f = fixture(t, slotOwner);
    f.write(receiverFile, receiver);
    assertUnresolved(f, f.add(SLOT, 'payload={'));
  });
}

semanticTest('rechecks changed receiver source instead of reusing a stale nonmemo proof', (t) => {
  const f = fixture(t, slotOwner);
  f.write(receiverFile, 'export function Surface({ payload }) { return <section>{payload}</section>; }');
  const diagnostic = f.add(SLOT, 'payload={');
  const proof = assertProof(f, f.evaluate(), diagnostic, 'nonmemoized-slot-receiver');
  f.write(receiverFile, `import { memo } from 'react'; export const Surface = memo(({ payload }) => <section>{payload}</section>);`);
  assert.notEqual(proof.evidence.find((entry) => entry.file === receiverFile).sha256, sourceDigest(f.sources.get(receiverFile)));
  assertUnresolved(f, diagnostic);
});

semanticTest('projects the exact ten real imported receiver diagnostics without review additions', (t) => {
  const prefix = 'web/app/src/';
  const slots = [
    ['features/frontstage/components/data-blocks/table/view/TableGrid.tsx', 'tools={', 0],
    ['features/frontstage/components/data-blocks/table/view/TableGrid.tsx', 'tools={', 1],
    ['features/frontstage/components/data-blocks/table/TableDesignControls.tsx', 'tools={', 0],
    ['features/frontstage/components/data-blocks/table/view/TableBlockView.tsx', 'fieldMenu={', 0],
    ['features/frontstage/components/data-blocks/table/view/TableBlockView.tsx', 'fieldMenu={', 1],
    ['features/frontstage/components/data-blocks/table/view/TableAction.tsx', 'tools={', 0],
    ['features/settings/components/ui-management/RegisteredBlocksTab.tsx', 'status={', 0],
    ['features/settings/components/ui-management/registered-blocks/RegisteredBlocksTable.tsx', 'filters={', 0],
    ['features/settings/pages/events/runtime/EventActivity.tsx', 'toolbarStart={', 0],
    ['features/settings/pages/events/runtime/QueueWorkbench.tsx', 'toolbarStart={', 0],
  ];
  const owners = [
    'features/frontstage/lib/native-modules/block-design/runtime.tsx',
    'features/frontstage/components/data-blocks/table/view/TableState.tsx',
    'features/settings/components/SettingsSectionSurface.tsx',
    'shared/ui/data-table/DataTableLayout.tsx',
    'features/settings/pages/events/EventsTable.tsx',
  ];
  const first = prefix + slots[0][0];
  const f = fixture(t, fs.readFileSync(path.join(ROOT, first), 'utf8'), first);
  for (const file of new Set([...slots.map(([file]) => file), ...owners])) {
    f.write(prefix + file, fs.readFileSync(path.join(ROOT, prefix + file), 'utf8'));
  }
  for (const [file, anchor, occurrence] of slots) f.add(SLOT, anchor, prefix + file, occurrence);
  const result = f.evaluate();
  assert.equal(result.upstreamCorrections.length, 10);
  for (const diagnostic of f.doctorReport.diagnostics) assertProof(f, result, diagnostic, 'nonmemoized-slot-receiver');
});

const stableRenderSource = `import { useState } from 'react';
function Editor() { const [count, setCount] = useState(0); return <button onClick={() => setCount(count + 1)}>{count}</button>; }
export function Panel({ tick }) {
  const renderEntry = () => <Editor />;
  return <section data-tick={tick}>{renderEntry()}</section>;
}`;
const dynamicRenderSource = `export function Panel({ tick }) {
  const renderEntry = () => { function Fresh() { return <input defaultValue="initial" />; } return <Fresh />; };
  return <section data-tick={tick}>{renderEntry()}</section>;
}`;

semanticTest('ordinary helper invocation returns a stable module component type', (t) => {
  const f = fixture(t, stableRenderSource);
  const diagnostic = f.add(RENDER, 'renderEntry()}</');
  assertProof(f, f.evaluate(), diagnostic, 'stable-render-result', ['const renderEntry', 'function Editor']);
});

for (const [name, source] of [
  ['fresh returned element type', dynamicRenderSource],
  ['helper splices hooks into caller', `import { useState } from 'react'; export function Panel() { const renderEntry = () => { const [value] = useState(0); return <button>{value}</button>; }; return <>{renderEntry()}</>; }`],
  ['opaque callback parameter', 'export function Layout({ renderEntry }) { return <section>{renderEntry()}</section>; }'],
  ['typed opaque ReactNode callback', `import type { ReactNode } from 'react'; export function Layout({ renderEntry }: { renderEntry(): ReactNode }) { return <section>{renderEntry()}</section>; }`],
  ['opaque returned factory type', 'export function Panel() { const renderEntry = () => { const Generated = manufacture(); return <Generated />; }; return <>{renderEntry()}</>; }'],
]) {
  semanticTest(`holds unproven render result: ${name}`, (t) => {
    const f = fixture(t, source);
    assertUnresolved(f, f.add(RENDER, 'renderEntry()}</'));
  });
}

const accessibleSource = `import { forwardRef } from 'react';
export const LabelledControl = forwardRef(function Control({ children, ...props }, ref) {
  const label = props['aria-label'] ?? (typeof children === 'string' ? children : undefined);
  return <button {...props} ref={ref} aria-label={label}>{children ?? label}</button>;
});`;

semanticTest('scalar narrowing supplies accessible naming while preserving actual children', (t) => {
  const f = fixture(t, accessibleSource);
  const diagnostic = f.add(CHILDREN, "typeof children === 'string'");
  assertProof(f, f.evaluate(), diagnostic, 'accessible-name-narrowing', ["props['aria-label']", 'aria-label={label}', 'children ?? label']);
});

semanticTest('checks every imported Tooltip naming consumer and its independent variant branch', (t) => {
  const f = fixture(t, `import { Tooltip as Hint } from 'antd'; export function Control({ children, variant, ...props }) {
    const name = props['aria-label'] ?? (typeof children === 'string' ? children : undefined);
    if (variant === 'wide') return <button aria-label={name}>{children ?? name}</button>;
    return <Hint title={name}><button aria-label={name}>{children}</button></Hint>;
  }`);
  const diagnostic = f.add(CHILDREN, "typeof children === 'string'");
  assertProof(f, f.evaluate(), diagnostic, 'accessible-name-narrowing', ['title={name}', 'aria-label={name}']);
});

for (const [name, body] of [
  ['type switches rendered subtree', `return typeof children === 'string' ? <span>{children}</span> : <div>{children}</div>;`],
  ['name controls disabled state', `const label = typeof children === 'string' ? children : undefined; return <button aria-label={label} disabled={!!label}>{children}</button>;`],
  ['name escapes to unknown consumer', `const label = typeof children === 'string' ? children : undefined; report(label); return <button aria-label={label}>{children}</button>;`],
  ['name controls class shape', `const label = typeof children === 'string' ? children : undefined; return <button aria-label={label} className={label}>{children}</button>;`],
  ['type-dependent component type', `const Kind = typeof children === 'string' ? TextControl : IconControl; return <Kind>{children}</Kind>;`],
  ['mutated scalar fallback', `let label = typeof children === 'string' ? children : undefined; label = transform(label); return <button aria-label={label}>{children}</button>;`],
  ['unknown tooltip contract', `const label = typeof children === 'string' ? children : undefined; return <Mystery title={label}>{children}</Mystery>;`],
  ['label replaces nonstring children', `const label = typeof children === 'string' ? children : undefined; return <button aria-label={label}>{label ?? 'replacement'}</button>;`],
]) {
  semanticTest(`does not excuse type-driven behavior or unknown dataflow: ${name}`, (t) => {
    const f = fixture(t, `export function Control({ children }) { ${body} }`);
    assertUnresolved(f, f.add(CHILDREN, "typeof children === 'string'"));
  });
}

semanticTest('supported React19 forwardRef genuinely consumes and forwards the ref', (t) => {
  const f = fixture(t, accessibleSource);
  const diagnostic = f.add(REF, 'forwardRef');
  assertProof(f, f.evaluate(), diagnostic, 'supported-forward-ref', ['ref={ref}', "from 'react'"]);
});

for (const [name, source] of [
  ['foreign wrapper', `import { forwardRef } from 'other'; export const Control = forwardRef((props, ref) => <button ref={ref} />);`],
  ['ignored ref', `import { forwardRef } from 'react'; export const Control = forwardRef((props, ref) => <button />);`],
  ['opaque render callback', `import { forwardRef } from 'react'; import { render } from 'unknown'; export const Control = forwardRef(render);`],
  ['locally shadowed invocation', `import { forwardRef } from 'react'; export function factory(forwardRef) { return forwardRef((props, ref) => <button ref={ref} />); }`],
]) {
  semanticTest(`requires actual supported ref plumbing: ${name}`, (t) => {
    const f = fixture(t, source);
    assertUnresolved(f, f.add(REF, 'forwardRef'));
  });
}

semanticTest('does not turn React19 support into a rule-wide migration exemption', (t) => {
  const f = fixture(t, accessibleSource);
  const diagnostic = f.add(REF, 'forwardRef');
  const original = { ...diagnostic };
  for (const change of [{ message: 'ReactDOM.render is removed' }, { rule: 'no-react-dom-deprecated-apis' },
    { plugin: 'other' }, { line: 9999 }, { column: 9999 }]) {
    Object.assign(diagnostic, original, change);
    assertUnresolved(f, diagnostic);
  }
  Object.assign(diagnostic, original);
  assertUnresolved(f, diagnostic, 'react-doctor@0.2.17');
  f.write('web/app/package.json', JSON.stringify({ dependencies: { react: '20.0.0' } }));
  assertUnresolved(f, diagnostic);
});

// These exercise real React reconciliation/ref behavior. They are controls for
// the diagnostic premise, not fake application objects or source snapshots.
function compileFixture(source) {
  const fromWeb = createRequire(path.join(ROOT, 'web/package.json'));
  const fromApp = createRequire(path.join(ROOT, 'web/app/package.json'));
  const ts = fromWeb('typescript');
  const output = ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX,
  } }).outputText;
  const module = { exports: {} };
  new Function('require', 'module', 'exports', output)(fromApp, module, module.exports);
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
  const { createRoot } = fromApp('react-dom/client');
  const mount = dom.window.document.getElementById('mount');
  const root = createRoot(mount);
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
  return { React, dom, mount, render: (element) => React.act(async () => root.render(element)) };
}

semanticTest('runtime: ordinary render helper keeps native node and child hook state', async (t) => {
  const view = await browser(t);
  const { Panel } = compileFixture(stableRenderSource);
  await view.render(view.React.createElement(Panel, { tick: 1 }));
  const button = view.mount.querySelector('button');
  await view.React.act(async () => button.dispatchEvent(new view.dom.window.MouseEvent('click', { bubbles: true })));
  assert.equal(button.textContent, '1');
  await view.render(view.React.createElement(Panel, { tick: 2 }));
  assert.equal(view.mount.querySelector('button'), button);
  assert.equal(button.textContent, '1', 'ordinary callback invocation does not reset child hook state');
});

semanticTest('runtime: freshly returned function element type really remounts', async (t) => {
  const view = await browser(t);
  const { Panel } = compileFixture(dynamicRenderSource);
  await view.render(view.React.createElement(Panel, { tick: 1 }));
  const original = view.mount.querySelector('input');
  original.value = 'unsaved draft';
  await view.render(view.React.createElement(Panel, { tick: 2 }));
  const replaced = view.mount.querySelector('input');
  assert.notEqual(replaced, original);
  assert.equal(replaced.value, 'initial', 'dynamic type is a real state-loss control that checker must hold');
});

semanticTest('runtime: current forwardRef passes the native ref and preserves text/element/absent children', async (t) => {
  const view = await browser(t);
  assert.match(view.React.version, /^19\./u);
  const { LabelledControl } = compileFixture(accessibleSource);
  const ref = view.React.createRef();
  await view.render(view.React.createElement(LabelledControl, { ref }, 'Save'));
  const button = view.mount.querySelector('button');
  assert.equal(ref.current, button);
  assert.equal(button.getAttribute('aria-label'), 'Save');
  assert.equal(button.textContent, 'Save');
  await view.render(view.React.createElement(LabelledControl, { ref, 'aria-label': 'Explicit name' },
    view.React.createElement('em', null, 'Actual child')));
  assert.equal(ref.current, button);
  assert.equal(button.getAttribute('aria-label'), 'Explicit name');
  assert.equal(button.querySelector('em').textContent, 'Actual child');
  await view.render(view.React.createElement(LabelledControl, { ref, 'aria-label': 'Fallback name' }));
  assert.equal(ref.current, button);
  assert.equal(button.textContent, 'Fallback name');
});

semanticTest('runtime: fresh JSX defeats memo but stable JSX does not', async (t) => {
  const view = await browser(t);
  let renders = 0;
  const Receiver = view.React.memo(function Receiver({ payload }) {
    renders++;
    return view.React.createElement('section', null, payload);
  });
  const stable = view.React.createElement('span', null, 'same content');
  await view.render(view.React.createElement(Receiver, { payload: stable }));
  await view.render(view.React.createElement(Receiver, { payload: stable }));
  assert.equal(renders, 1, 'memo can skip a stable prop');
  await view.render(view.React.createElement(Receiver, {
    payload: view.React.createElement('span', null, 'same content'),
  }));
  assert.equal(renders, 2, 'a fresh element is a real bailout failure for memoized receivers');
});
