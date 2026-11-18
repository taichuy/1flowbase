const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {
  REVIEW_FILE,
  sourceDigest,
  evaluateDiagnosticReviews,
} = require('../diagnostic-review.js');

const TOOL = 'react-doctor@0.2.16';
const TOOL_INTEGRITY = 'sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==';
// npm SLSA provenance and the annotated oxlint-plugin-react-doctor@0.2.16 tag
// independently resolve to this source. These are the original faulty rules.
const PINNED_SOURCE = '2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc';
const RULE_SOURCE_SHA = {
  'no-derived-state': 'db98ba7989c837bb635b3fed080a937ed425ecce6330126dd58b15f66e9eb91f',
  'no-event-handler': '1f89d763b79829a89a3c00e9d655ff8299e26b98c5f7d46a770f3bb0ccf74062',
};
const ACCUMULATOR_COUNTEREXAMPLE = 'https://github.com/millionco/react-doctor/pull/1017';
const EXTERNAL_WIDGET_CONTRACT = 'https://react.dev/reference/react/useEffect#controlling-a-non-react-widget';
const eventMessage = 'Faking an event handler with a prop plus a useEffect costs an extra render & runs late.';
const semanticTest = (name, fn) => test(`effect semantic correction: ${name}`, fn);

function fixture(t, source, file = 'web/app/src/renamed-owner.tsx') {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'doctor-effect-proof-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const sources = new Map();
  const write = (name, body) => {
    fs.mkdirSync(path.dirname(path.join(repoRoot, name)), { recursive: true });
    fs.writeFileSync(path.join(repoRoot, name), body);
    sources.set(name, body);
  };
  write(file, source);
  const manifest = { schemaVersion: 1, toolPackage: TOOL, reviews: [] };
  write(REVIEW_FILE, JSON.stringify(manifest));
  const doctorReport = {
    diagnostics: [],
    summary: { totalDiagnosticCount: 0, suppressedDiagnosticCount: 0 },
    error: null,
  };
  const add = (rule, anchor, state = 'samples') => {
    const offset = sources.get(file).indexOf(anchor);
    assert.ok(offset >= 0, `diagnostic anchor must exist: ${anchor}`);
    const before = sources.get(file).slice(0, offset);
    const diagnostic = {
      filePath: file.slice('web/app/'.length), plugin: 'react-doctor', rule,
      severity: 'warning', line: before.split('\n').length,
      column: before.length - before.lastIndexOf('\n'),
      message: rule === 'no-derived-state'
        ? `Storing "${state}" in state when you can derive it from other values costs an extra render.`
        : eventMessage,
    };
    doctorReport.diagnostics.push(diagnostic);
    doctorReport.summary.totalDiagnosticCount += 1;
    return diagnostic;
  };
  const evaluate = (toolPackage = TOOL) => {
    const raw = structuredClone(doctorReport);
    const result = evaluateDiagnosticReviews({ repoRoot, doctorReport, toolPackage });
    assert.deepEqual(doctorReport, raw, 'corrections must preserve the full raw diagnostic inventory, totals and severities');
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))), manifest, 'no review or identity exception may be written');
    return result;
  };
  return { repoRoot, file, sources, write, doctorReport, add, evaluate };
}

function assertProof(f, result, diagnostic, predicate, proofTexts) {
  assert.equal(result.status, 'reviewed');
  assert.equal(result.findings.length, 0);
  assert.equal(result.upstreamCorrections.length, 1);
  const correction = result.upstreamCorrections[0];
  assert.deepEqual(correction.diagnostic, diagnostic);
  assert.equal(correction.predicate, predicate);
  assert.ok(correction.reason.length > 20);
  assert.equal(correction.provenance.toolPackage, TOOL);
  assert.equal(correction.provenance.packageIntegrity, TOOL_INTEGRITY);
  assert.equal(correction.provenance.sourceCommit, PINNED_SOURCE);
  assert.equal(correction.provenance.ruleSourceSha256, RULE_SOURCE_SHA[diagnostic.rule]);
  assert.ok(correction.provenance.references.includes(
    predicate === 'history-accumulator' ? ACCUMULATOR_COUNTEREXAMPLE : EXTERNAL_WIDGET_CONTRACT
  ));
  assert.ok(Array.isArray(correction.evidence) && correction.evidence.length > 0);
  for (const evidence of correction.evidence) {
    assert.ok(f.sources.has(evidence.file), 'proof must reference an actual inspected source');
    const source = f.sources.get(evidence.file);
    assert.equal(evidence.sha256, sourceDigest(source));
    assert.ok(Array.isArray(evidence.spans) && evidence.spans.length > 0);
    for (const span of evidence.spans) {
      assert.ok(Number.isInteger(span.start) && Number.isInteger(span.end));
      assert.ok(span.start >= 0 && span.start < span.end && span.end <= source.length);
    }
  }
  for (const { file, text } of proofTexts) {
    const evidence = correction.evidence.find((entry) => entry.file === file);
    assert.ok(evidence, `missing proof source: ${file}`);
    assert.ok(evidence.spans.some((span) => f.sources.get(file).slice(span.start, span.end) === text), 'proof must retain the exact relevant AST span');
  }
  return correction;
}

function historySource(updater, initial = '[]', rendered = 'samples.length') {
  return `import { useEffect, useState } from 'react';
export function Timeline({ sample }) {
  const [samples, setSamples] = useState(${initial});
  useEffect(() => { ${updater}; }, [sample]);
  return ${rendered};
}
`;
}

for (const [name, call, initial, rendered] of [
  ['array history', 'setSamples(previous => [...previous, sample])'],
  ['running total', 'setSamples(previous => previous + sample)', '0', 'samples'],
  ['block return', 'setSamples(previous => { return [...previous, sample]; })'],
]) {
  semanticTest(`accepts ${name} with bound return dependence and exact proof`, (t) => {
    const f = fixture(t, historySource(call, initial, rendered));
    const diagnostic = f.add('no-derived-state', call);
    assertProof(f, f.evaluate(), diagnostic, 'history-accumulator', [{ file: f.file, text: call }]);
  });
}

const appendFunction = `export function rememberEarlier(records, incoming) {
  if (records.at(-1)?.captured_at === incoming.captured_at) return records;
  return [...records.filter(point => point.captured_at >= incoming.captured_at - 1800000), incoming].slice(-361);
}`;
const helperCall = 'setSamples(previous => rememberEarlier(previous, sample))';
const helperSource = `import { useEffect, useState } from 'react';
import { rememberEarlier } from './arbitrary-projection';
export function RenamedOwner({ sample }) {
  const [samples, setSamples] = useState([]);
  useEffect(() => { ${helperCall}; }, [sample]);
  return samples.length;
}
`;
semanticTest('follows a renamed relative helper and records both source digests and AST spans', (t) => {
  const f = fixture(t, helperSource, 'web/app/src/not-a-statistics-component.tsx');
  const helperFile = 'web/app/src/arbitrary-projection.ts';
  f.write(helperFile, `${appendFunction}\nexport const unrelated = 1;\n`);
  const diagnostic = f.add('no-derived-state', helperCall);
  assertProof(f, f.evaluate(), diagnostic, 'history-accumulator', [
    { file: f.file, text: helperCall },
    { file: helperFile, text: appendFunction },
  ]);
  f.write(helperFile, 'export function rememberEarlier(records, incoming) { return [incoming]; }\n');
  assert.equal(f.evaluate().status, 'requires-review', 'changing helper semantics invalidates proof without relying on a stored identity digest');
});

for (const [name, call] of [
  ['direct prop copy', 'setSamples(sample)'],
  ['ignored updater parameter', 'setSamples(previous => sample)'],
  ['zero-parameter prop copy', 'setSamples(() => sample)'],
  ['shadowed parameter', 'setSamples(previous => sample.map(previous => previous.value))'],
  ['discarded prior read', 'setSamples(previous => { console.log(previous); return sample; })'],
  ['equality-only prior read', 'setSamples(previous => previous === sample ? previous : sample)'],
  ['spread-only prop overwrite', 'setSamples(previous => ({ ...previous, value: sample }))'],
  ['opaque helper', 'setSamples(previous => unknownProjection(previous, sample))'],
  ['mutated prior array', 'setSamples(previous => { previous.push(sample); return previous; })'],
]) {
  semanticTest(`keeps genuine or unproved derived-state case unresolved: ${name}`, (t) => {
    const f = fixture(t, historySource(call));
    f.add('no-derived-state', call);
    const result = f.evaluate();
    assert.equal(result.status, 'requires-review');
    assert.equal(result.reviewed, 0);
    assert.equal(result.upstreamCorrections?.length ?? 0, 0);
  });
}

for (const [name, body] of [
  ['QA vacuous filter', '[...previous.filter(item => !!item && false), sample]'],
  ['correlated filter contradiction', '[...previous.filter(item => !!item && !item), sample]'],
  ['contradictory selector comparisons', '[...previous.filter(item => item.captured_at > 2 && item.captured_at < 1), sample]'],
  ['QA self length slice', '[...previous.slice(previous.length), sample]'],
  ['equal slice bounds', '[...previous.slice(1, 1), sample]'],
  ['slice only newly appended element', '[...previous, sample].slice(-1)'],
  ['slice after all prior elements', '[...previous, sample].slice(previous.length)'],
  ['empty chained slice', '[...previous.slice(0, 1).slice(1), sample]'],
]) {
  semanticTest(`rejects history with no retained prior element: ${name}`, (t) => {
    const call = `setSamples(previous => ${body})`;
    const f = fixture(t, historySource(call));
    f.add('no-derived-state', call);
    const result = f.evaluate();
    assert.equal(result.status, 'requires-review');
    assert.equal(result.upstreamCorrections?.length ?? 0, 0);
  });
}

for (const [name, call] of [
  ['constant alias false filter', 'setSamples(previous => { const never = false; return [...previous.filter(item => !!item && never), sample]; })'],
  ['self length alias slice', 'setSamples(previous => { const start = previous.length; return [...previous.slice(start), sample]; })'],
  ['zero slice end alias', 'setSamples(previous => { const end = 0; return [...previous.slice(0, end), sample]; })'],
]) {
  semanticTest(`rejects vacuous history through immutable aliases: ${name}`, (t) => {
    const f = fixture(t, historySource(call));
    f.add('no-derived-state', call);
    assert.equal(f.evaluate().status, 'requires-review');
  });
}

for (const body of [
  '[...previous.filter(item => !!item && true), sample]',
  '[...previous.slice(1), sample]',
  '[...previous, sample].slice(-2)',
]) {
  semanticTest(`retains a genuine prior-element witness: ${body}`, (t) => {
    const call = `setSamples(previous => ${body})`;
    const f = fixture(t, historySource(call));
    const diagnostic = f.add('no-derived-state', call);
    assertProof(f, f.evaluate(), diagnostic, 'history-accumulator', [{ file: f.file, text: call }]);
  });
}

for (const [component, helper] of [
  ['ObservationStatistics', 'observation-statistics'],
  ['QueueStatistics', 'queue-statistics'],
]) {
  semanticTest(`retains actual time-window history consumer ${component}`, (t) => {
    const directory = 'web/app/src/features/settings/pages/events/runtime/statistics';
    const file = `${directory}/${component}.tsx`;
    const helperFile = `${directory}/${helper}.ts`;
    const repoRoot = path.resolve(__dirname, '../../../..');
    const f = fixture(t, fs.readFileSync(path.join(repoRoot, file), 'utf8'), file);
    f.write(helperFile, fs.readFileSync(path.join(repoRoot, helperFile), 'utf8'));
    const diagnostic = f.add('no-derived-state', 'setSamples((previous) =>');
    const correction = assertProof(f, f.evaluate(), diagnostic, 'history-accumulator', []);
    assert.ok(correction.evidence.some((entry) => entry.file === helperFile), 'the actual append helper must be proved');
  });
}

semanticTest('does not authorize another setter in the same history effect', (t) => {
  const history = 'setSamples(previous => [...previous, sample])';
  const mirror = 'setLabel(sample.name)';
  const source = `import { useEffect, useState } from 'react';
function Timeline({ sample }) {
  const [samples, setSamples] = useState([]);
  const [label, setLabel] = useState('');
  useEffect(() => { ${history}; ${mirror}; }, [sample]);
  return label;
}`;
  const f = fixture(t, source);
  const diagnostic = f.add('no-derived-state', history);
  const violation = f.add('no-derived-state', mirror, 'label');
  const result = f.evaluate();
  assert.equal(result.status, 'requires-review');
  assert.equal(result.reviewed, 1);
  assert.deepEqual(result.upstreamCorrections.map((entry) => entry.diagnostic), [diagnostic]);
  assert.equal(result.findings.length, 1);
  assert.equal(result.findings[0].line, violation.line);
});

const formCall = 'editor.setFieldsValue({ name: receipt.data.name, tag_ids: receipt.data.tags.map(tag => tag.id) })';
const queryHydration = `import { useEffect } from 'react';
import { Form as ExternalForm } from 'antd';
import { useQuery } from '@tanstack/react-query';
import { loadRecord } from './api';
export function Draft({ open, id }) {
  const [editor] = ExternalForm.useForm();
  const receipt = useQuery({ queryKey: ['record', id], queryFn: () => loadRecord(id), enabled: open });
  useEffect(() => { if (receipt.isSuccess) ${formCall}; }, [editor, receipt.data, receipt.isSuccess]);
  return null;
}`;
semanticTest('accepts proven AntD Form synchronization after an async query receipt', (t) => {
  const f = fixture(t, queryHydration);
  // The pinned rule follows query inputs and may report an upstream declaration,
  // rather than the imperative statement or even a node inside useEffect.
  const diagnostic = f.add('no-event-handler', 'open });');
  assertProof(f, f.evaluate(), diagnostic, 'external-form-synchronization', [{ file: f.file, text: formCall }]);
});

semanticTest('accepts controlled-open reset and hydration with a local session-ref reset', (t) => {
  const reset = 'editor.resetFields()';
  const hydrate = 'editor.setFieldsValue(defaults)';
  const source = `import { useEffect, useRef } from 'react';
import { Form } from 'antd';
export function Editor({ open, defaults }) {
  const [editor] = Form.useForm();
  const pending = useRef(null);
  useEffect(() => {
    if (!open) { ${reset}; pending.current = null; return; }
    ${hydrate};
  }, [open, defaults, editor]);
  return null;
}`;
  const f = fixture(t, source, 'web/app/src/arbitrary-editor.tsx');
  const diagnostic = f.add('no-event-handler', 'open) {');
  assertProof(f, f.evaluate(), diagnostic, 'external-form-synchronization', [
    { file: f.file, text: reset }, { file: f.file, text: hydrate },
  ]);
});

function eventSource(body, declarations = 'const [editor] = Form.useForm();') {
  return `import { useEffect } from 'react';
import { Form } from 'antd';
export function Editor({ ready, receipt, replacement }) {
  ${declarations}
  useEffect(() => { if (ready) { ${body} } }, [ready, receipt, editor]);
  return null;
}`;
}
for (const [name, body, declarations] of [
  ['notification relay', 'notify(receipt);'],
  ['network write', 'saveApi(receipt);'],
  ['form submission event', 'editor.submit();'],
  ['mixed analytics', 'editor.setFieldsValue(receipt); analytics.track("loaded");'],
  ['mixed callback', 'editor.setFieldsValue(receipt); onSaved();'],
  ['effectful form argument', 'editor.setFieldsValue({ name: saveApi() });'],
  ['fake form receiver', 'editor.setFieldsValue(receipt);', 'const editor = { setFieldsValue(value) { return value; } };'],
  ['reassigned form receiver', 'editor.setFieldsValue(receipt);', 'let [editor] = Form.useForm(); editor = replacement;'],
  ['opaque hydration helper', 'hydrate(editor, receipt);'],
]) {
  semanticTest(`keeps genuine or unproved event case unresolved: ${name}`, (t) => {
    const f = fixture(t, eventSource(body, declarations));
    f.add('no-event-handler', 'ready) {');
    const result = f.evaluate();
    assert.equal(result.status, 'requires-review');
    assert.equal(result.reviewed, 0);
    assert.equal(result.upstreamCorrections?.length ?? 0, 0);
  });
}

semanticTest('fails closed when an upstream guard also drives a different event effect', (t) => {
  const source = eventSource('editor.setFieldsValue(receipt);').replace(
    '  return null;',
    '  useEffect(() => { if (ready) notify(receipt); }, [ready, receipt]);\n  return null;'
  );
  const f = fixture(t, source);
  f.add('no-event-handler', 'ready) {');
  assert.equal(f.evaluate().status, 'requires-review');
});

semanticTest('fails closed on unknown tool identity, message, syntax, location or missing source', (t) => {
  const call = 'setSamples(previous => [...previous, sample])';
  const f = fixture(t, historySource(call));
  const diagnostic = f.add('no-derived-state', call);
  assert.equal(f.evaluate('react-doctor@future').status, 'requires-review');
  const original = structuredClone(diagnostic);
  for (const mutation of [
    { plugin: 'unknown-plugin' },
    { rule: 'unknown-rule' },
    { message: 'a different finding from an unknown rule revision' },
    { line: 99999 },
    { column: 99999 },
    { filePath: '../../outside.tsx' },
  ]) {
    Object.assign(diagnostic, original, mutation);
    assert.equal(f.evaluate().status, 'requires-review');
  }
  Object.assign(diagnostic, original);
  f.write(f.file, 'export const invalid = (');
  assert.equal(f.evaluate().status, 'requires-review');
  fs.unlinkSync(path.join(f.repoRoot, f.file));
  assert.equal(f.evaluate().status, 'requires-review');
});

const { receiverThreats, nativeReceivers } = require('./fixtures/native-receiver-runtime.cjs');

function formReceiverSource(patch = '', receiver = 'editor', consumer = 'null',
  formImport = "import { Form } from 'antd';", factory = 'Form.useForm()') {
  return `import { useEffect } from 'react';
${formImport}
function Owner({ receipt }) {
  const [editor] = ${factory};
  ${patch}
  useEffect(() => { ${receiver}.setFieldsValue(receipt); }, [receipt]);
  return ${consumer};
}
`;
}

function assertFormReceiverUnproved(f) {
  f.add('no-event-handler', 'receipt);');
  const result = f.evaluate();
  assert.equal(result.status, 'requires-review');
  assert.equal(result.reviewed, 0);
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
  assert.equal(result.findings.length, 1);
}

for (const [name, patch] of receiverThreats('editor', 'setFieldsValue', 'patchForm')) {
  semanticTest(`requires unchanged native Form provenance: ${name}`, (t) => {
    const f = fixture(t, formReceiverSource(patch));
    assertFormReceiverUnproved(f);
  });
}

for (const [name, patch, receiver = 'editor', consumer = 'null', formImport, factory] of [
  ['native receiver', ''],
  ['immutable receiver aliases', 'const first = editor; const second = first;', 'second'],
  ['standard imported AntD Form consumer', '', 'editor', '<Form form={editor}><Form.Item name="value"><input /></Form.Item></Form>'],
  ['renamed native Form import consumer', '', 'editor', '<AntForm form={editor} />', "import { Form as AntForm } from 'antd';", 'AntForm.useForm()'],
  ['immutable native component alias consumer', 'const NativeForm = Form;', 'editor', '<NativeForm form={editor} />'],
  ['same-named unrelated mutations', `const unrelated = { setFieldsValue: submit };
    Object.assign(unrelated, { setFieldsValue: submit });
    Object.defineProperty(unrelated, 'setFieldsValue', { value: submit });
    Reflect.set(unrelated, 'setFieldsValue', submit); patchOther(unrelated);`],
  ['shadowed Object only sees another receiver', 'const unrelated = {}; const Object = { keys: patchForm }; Object.keys(unrelated);'],
]) {
  semanticTest(`retains native Form control: ${name}`, (t) => {
    const f = fixture(t, formReceiverSource(patch, receiver, consumer, formImport, factory));
    const diagnostic = f.add('no-event-handler', 'receipt);');
    assertProof(f, f.evaluate(), diagnostic, 'external-form-synchronization', [{ file: f.file, text: `${receiver}.setFieldsValue(receipt)` }]);
  });
}

for (const [name, patch, consumer] of [
  ['opaque custom JSX consumer', 'const Custom = ({ form }) => { patchForm(form); return null; };', '<Custom form={editor} />'],
  ['native-looking custom JSX alias', 'const NativeForm = OpaqueForm;', '<NativeForm form={editor} />'],
]) {
  semanticTest(`holds an unproved Form receiver consumer: ${name}`, (t) => {
    const f = fixture(t, formReceiverSource(patch, 'editor', consumer));
    assertFormReceiverUnproved(f);
  });
}

const formNativeCases = [
  ['actual native form store action', '', 'editor', true],
  ['immutable alias native action', 'const alias = editor;', 'alias', true],
  ['direct assignment negative control', 'editor.setFieldsValue = submit;', 'editor', false],
  ['QA Object.assign form countermodel', 'Object.assign(editor, { setFieldsValue: submit });', 'editor', false],
  ['QA descriptor form countermodel', 'Object.defineProperty(editor, "setFieldsValue", { value: submit });', 'editor', false],
  ['QA opaque form countermodel', 'patchForm(editor);', 'editor', false],
].map(([name, patch, receiver, native]) => ({ name, source: formReceiverSource(patch, receiver), native }));
let formNativeObservations;
for (const [index, entry] of formNativeCases.entries()) {
  semanticTest(`installed AntD FormStore runtime evidence: ${entry.name}`, (t) => {
    formNativeObservations ??= nativeReceivers('form', formNativeCases);
    const observed = formNativeObservations[index];
    assert.equal(observed.sourceSha256, sourceDigest(entry.source), 'execute the same source inspected by the gate');
    assert.match(observed.library, /@rc-component[+\/]form/u);
    assert.equal(observed.before, 0, 'the actual installed FormStore starts with value zero');
    assert.equal(observed.value, entry.native ? 1 : 0, 'inspect actual AntD FormStore data');
    assert.equal(observed.businessSubmits, entry.native ? 0 : 1, 'a substituted method dispatches business work instead of hydration');
    const f = fixture(t, entry.source);
    if (entry.native) {
      const diagnostic = f.add('no-event-handler', 'receipt);');
      assertProof(f, f.evaluate(), diagnostic, 'external-form-synchronization', []);
    } else assertFormReceiverUnproved(f);
  });
}

semanticTest('holds a native Form factory with an unproved supplied receiver', (t) => {
  const source = formReceiverSource().replace('function Owner({ receipt })', 'function Owner({ receipt, foreign })')
    .replace('Form.useForm();', 'Form.useForm(foreign);');
  assertFormReceiverUnproved(fixture(t, source));
});

semanticTest('holds a patched native Form factory surface before construction', (t) => {
  const source = formReceiverSource().replace('function Owner({ receipt })', 'function Owner({ receipt, foreign })')
    .replace('  const [editor] = Form.useForm();', '  Object.assign(Form, { useForm: () => [foreign] });\n  const [editor] = Form.useForm();');
  assertFormReceiverUnproved(fixture(t, source));
});

semanticTest('holds an extracted Form method without its inspected receiver', (t) => {
  const source = formReceiverSource('const { setFieldsValue: hydrate } = editor;')
    .replace('editor.setFieldsValue(receipt);', 'hydrate(receipt);');
  assertFormReceiverUnproved(fixture(t, source));
});
