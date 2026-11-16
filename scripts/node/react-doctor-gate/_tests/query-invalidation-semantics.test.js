const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { REVIEW_FILE, sourceDigest, evaluateDiagnosticReviews } = require('../diagnostic-review.js');

const TOOL = 'react-doctor@0.2.16';
const RULE = 'query-mutation-missing-invalidation';
const MESSAGE = 'useMutation with no cache update leaves your users looking at stale data after it runs.';
const COMMIT = '2d1ab245f0cbaa87529067e6e3ed9ac08362d1fc';
const RULE_SHA = 'e2803ee846be2117441a2edac81120838a0f62b0e3f306b088b262b3f2ffca9c';
const INTEGRITY = 'sha512-o108QscqM/TlC1QmKgDORf/P53PeZ69nZbRsvy53rXkKvWdKZC1Y0u1JRPmThbTh6v+QpMispBwEnFHhlpZQ7w==';
const UPSTREAM = `https://github.com/millionco/react-doctor/blob/${COMMIT}/packages/oxlint-plugin-react-doctor/src/plugin/rules/tanstack-query/${RULE}.ts`;
const semanticTest = (name, run) => test(`query invalidation semantic correction: ${name}`, run);

function fixture(t, source, file = 'web/app/src/renamed-workbench.tsx') {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'doctor-query-proof-'));
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
    diagnostics: [], summary: { totalDiagnosticCount: 0, suppressedDiagnosticCount: 0 }, error: null,
  };
  const add = (anchor = 'useMutation({', occurrence = 0, target = file) => {
    const source = sources.get(target);
    let offset = -1;
    for (let index = 0; index <= occurrence; index++) offset = source.indexOf(anchor, offset + 1);
    assert.ok(offset >= 0, `missing diagnostic anchor ${anchor} in ${target}`);
    const before = source.slice(0, offset);
    const diagnostic = {
      filePath: target.slice('web/app/'.length), plugin: 'react-doctor', rule: RULE,
      severity: 'warning', message: MESSAGE,
      line: before.split('\n').length, column: before.length - before.lastIndexOf('\n'),
    };
    doctorReport.diagnostics.push(diagnostic);
    doctorReport.summary.totalDiagnosticCount++;
    return diagnostic;
  };
  const evaluate = (toolPackage = TOOL) => {
    const raw = structuredClone(doctorReport);
    const beforeSources = new Map(sources);
    const result = evaluateDiagnosticReviews({ repoRoot, doctorReport, toolPackage });
    assert.deepEqual(doctorReport, raw, 'retain complete raw diagnostics, counts, positions and severities');
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(repoRoot, REVIEW_FILE))), manifest,
      'no file, helper, key or rule exception may be written');
    for (const [name, body] of beforeSources) {
      assert.equal(fs.readFileSync(path.join(repoRoot, name), 'utf8'), body, 'proof must not edit its sources');
    }
    return result;
  };
  return { repoRoot, file, sources, write, doctorReport, add, evaluate };
}

function assertProof(f, result, diagnostics, snippets = []) {
  assert.equal(result.status, 'reviewed');
  assert.equal(result.findings.length, 0);
  assert.equal(result.upstreamCorrections.length, diagnostics.length);
  assert.equal(result.reviewed, diagnostics.length);
  for (const diagnostic of diagnostics) {
    const proof = result.upstreamCorrections.find((item) => item.diagnostic === diagnostic
      || JSON.stringify(item.diagnostic) === JSON.stringify(diagnostic));
    assert.ok(proof, 'each raw diagnostic needs its own source-bound proof');
    assert.equal(proof.predicate, 'mutation-success-invalidation');
    assert.equal(proof.provenance.toolPackage, TOOL);
    assert.equal(proof.provenance.packageIntegrity, INTEGRITY);
    assert.equal(proof.provenance.sourceCommit, COMMIT);
    assert.equal(proof.provenance.ruleSourceSha256, RULE_SHA);
    assert.ok(proof.provenance.references.includes(UPSTREAM));
    assert.ok(proof.reason.length > 20);
    assert.ok(Array.isArray(proof.evidence) && proof.evidence.length > 0);
    for (const evidence of proof.evidence) {
      const source = f.sources.get(evidence.file);
      assert.equal(typeof source, 'string', 'evidence must be an inspected file');
      assert.equal(evidence.sha256, sourceDigest(source));
      assert.ok(evidence.spans.length > 0);
      for (const span of evidence.spans) {
        assert.ok(Number.isInteger(span.start) && Number.isInteger(span.end));
        assert.ok(span.start >= 0 && span.start < span.end && span.end <= source.length);
      }
    }
    for (const snippet of snippets) {
      assert.ok(proof.evidence.some((evidence) => evidence.spans.some((span) =>
        f.sources.get(evidence.file).slice(span.start, span.end).includes(snippet))),
      `missing executed source evidence: ${snippet}`);
    }
  }
}

function assertUnresolved(f, diagnostic, toolPackage = TOOL) {
  const result = f.evaluate(toolPackage);
  assert.equal(result.status, 'requires-review');
  assert.equal(result.findings.length, 1);
  assert.equal(result.findings[0].rule, `${diagnostic.plugin}/${diagnostic.rule}`);
  assert.equal(result.upstreamCorrections?.length ?? 0, 0);
}

const imports = `import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback } from 'react';`;
const client = 'const client = useQueryClient();';
const invalidate = 'client.invalidateQueries({ queryKey: itemsKey })';
const helper = `const synchronize = async () => { await ${invalidate}; };`;
function owner(body, header = imports) {
  return `${header}
export function Workbench({ shouldRefresh, save, notify, sourceKey }) {
  const itemsKey = ['items'];
  ${client}
  useQuery({ queryKey: [...itemsKey, 'list'], queryFn: () => fetch('/items') });
  ${body}
}`;
}

semanticTest('follows one renamed lexical helper for three successful writes', (t) => {
  const f = fixture(t, owner(`${helper}
    const first = useMutation({ mutationFn: save, onSuccess: synchronize });
    const second = useMutation({ mutationFn: save, onSuccess: synchronize });
    const third = useMutation({ mutationFn: save, onSuccess: synchronize });`));
  const diagnostics = [f.add(), f.add('useMutation({', 1), f.add('useMutation({', 2)];
  assertProof(f, f.evaluate(), diagnostics, ['useQueryClient()', invalidate, 'onSuccess: synchronize']);
});

semanticTest('follows awaited useCallback helper after a UI-only branch', (t) => {
  const f = fixture(t, owner(`const synchronize = useCallback(async () => {
      await Promise.all([${invalidate}, client.invalidateQueries({ queryKey: ['catalog'] })]);
    }, [client]);
    const write = useMutation({ mutationFn: save, onSuccess: async () => {
      if (shouldRefresh) { notify(); }
      await synchronize();
    } });`));
  const diagnostic = f.add();
  assertProof(f, f.evaluate(), [diagnostic], ['useCallback(', 'await synchronize()', invalidate]);
});

semanticTest('retains an unconditional target update alongside optional source invalidation', (t) => {
  const f = fixture(t, owner(`const synchronize = async () => {
      await Promise.all([${invalidate}, ...(sourceKey ? [client.invalidateQueries({ queryKey: sourceKey })] : [])]);
    };
    const write = useMutation({ mutationFn: save, onSuccess: synchronize });`));
  const diagnostic = f.add();
  assertProof(f, f.evaluate(), [diagnostic], [invalidate, 'Promise.all(']);
});

semanticTest('resolves imported hook aliases and immutable helper aliases without name allowlists', (t) => {
  const source = `import { useMutation as changeRemote, useQueryClient as currentCache } from '@tanstack/react-query';
import { useCallback as stable } from 'react';
export function RenamedPanel({ save }) {
  const cache = currentCache();
  const synchronize = stable(async () => { await cache.invalidateQueries({ queryKey: ['items'] }); }, [cache]);
  const alias = synchronize;
  const write = changeRemote({ mutationFn: save, onSuccess: alias });
}`;
  const f = fixture(t, source, 'web/app/src/other-feature/renamed-source.ts');
  const diagnostic = f.add('changeRemote({');
  assertProof(f, f.evaluate(), [diagnostic], ['currentCache()', 'cache.invalidateQueries(', 'stable(']);
});

semanticTest('projects the actual five diagnostics with source receipts and unchanged raw inventory', (t) => {
  const repository = path.resolve(__dirname, '../../../..');
  const data = 'web/app/src/features/frontstage/api/data-blocks.ts';
  const blocks = 'web/app/src/features/settings/components/ui-management/RegisteredBlocksTab.tsx';
  const f = fixture(t, fs.readFileSync(path.join(repository, data), 'utf8'), data);
  f.write(blocks, fs.readFileSync(path.join(repository, blocks), 'utf8'));
  const diagnostics = [f.add(), f.add('useMutation({', 1), f.add('useMutation({', 2),
    f.add('useMutation({', 0, blocks), f.add('useMutation({', 1, blocks)];
  assertProof(f, f.evaluate(), diagnostics);
});

// Every counterexample still writes server state. A helper name or a cache-like
// method without an executed success-path invalidation cannot repair stale reads.
for (const [name, body] of [
  ['missing success callback', 'useMutation({ mutationFn: save });'],
  ['same-named noop helper', 'const synchronize = () => {}; useMutation({ mutationFn: save, onSuccess: synchronize });'],
  ['unused helper', `${helper} useMutation({ mutationFn: save, onSuccess: () => notify() });`],
  ['returned helper value', `${helper} useMutation({ mutationFn: save, onSuccess: () => synchronize });`],
  ['uncalled nested callback', `${helper} useMutation({ mutationFn: save, onSuccess: () => { const nested = () => synchronize(); } });`],
  ['error-only invalidation', `${helper} useMutation({ mutationFn: save, onError: synchronize });`],
  ['pre-write invalidation', `${helper} useMutation({ mutationFn: save, onMutate: synchronize });`],
  ['early success exit', `${helper} useMutation({ mutationFn: save, onSuccess: async () => { if (!shouldRefresh) return; await synchronize(); } });`],
  ['conditional-only invalidation', `${helper} useMutation({ mutationFn: save, onSuccess: async () => { if (shouldRefresh) await synchronize(); } });`],
  ['conditional-only helper effect', `const synchronize = async () => { if (shouldRefresh) await ${invalidate}; }; useMutation({ mutationFn: save, onSuccess: synchronize });`],
  ['deferred unawaited callback', `${helper} useMutation({ mutationFn: save, onSuccess: () => setTimeout(() => synchronize(), 0) });`],
  ['mutated helper', `let synchronize = async () => { await ${invalidate}; }; synchronize = () => {}; useMutation({ mutationFn: save, onSuccess: synchronize });`],
  ['shadowed helper', `${helper} { const synchronize = () => {}; useMutation({ mutationFn: save, onSuccess: synchronize }); }`],
  ['mutated client', `let cache = client; cache = { invalidateQueries() {} }; const synchronize = () => cache.invalidateQueries({ queryKey: itemsKey }); useMutation({ mutationFn: save, onSuccess: synchronize });`],
  ['shadowed client', `const synchronize = () => { const client = { invalidateQueries() {} }; return ${invalidate}; }; useMutation({ mutationFn: save, onSuccess: synchronize });`],
  ['unrelated cache-like receiver', 'const mirror = { invalidateQueries() {} }; const synchronize = () => mirror.invalidateQueries({ queryKey: itemsKey }); useMutation({ mutationFn: save, onSuccess: synchronize });'],
  ['cyclic helper graph', 'const first = () => second(); const second = () => first(); useMutation({ mutationFn: save, onSuccess: first });'],
  ['cancel-only helper', 'const synchronize = () => client.cancelQueries({ queryKey: itemsKey }); useMutation({ mutationFn: save, onSuccess: synchronize });'],
  ['uninvoked Promise array callback', `const synchronize = async () => { await Promise.all([() => ${invalidate}]); }; useMutation({ mutationFn: save, onSuccess: synchronize });`],
  ['shadowed Promise', `const Promise = { all() {} }; const synchronize = async () => { await Promise.all([() => ${invalidate}]); }; useMutation({ mutationFn: save, onSuccess: synchronize });`],
  ['overridden success callback', `${helper} useMutation({ mutationFn: save, onSuccess: synchronize, ...shouldRefresh });`],
  ['duplicate success callback', `${helper} useMutation({ mutationFn: save, onSuccess: synchronize, onSuccess: () => {} });`],
]) {
  semanticTest(`keeps genuine stale path unresolved: ${name}`, (t) => {
    const f = fixture(t, owner(body));
    assertUnresolved(f, f.add());
  });
}

for (const [name, header] of [
  ['local query client hook', `import { useMutation, useQuery } from '@tanstack/react-query'; import { useCallback } from 'react'; const useQueryClient = () => ({ invalidateQueries() {} });`],
  ['unrelated hook package', `import { useMutation, useQuery } from '@tanstack/react-query'; import { useCallback } from 'react'; import { useQueryClient } from './unrelated';`],
  ['local callback wrapper', `import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'; const useCallback = () => () => {};`],
  ['local mutation hook', `import { useQuery, useQueryClient } from '@tanstack/react-query'; import { useCallback } from 'react'; const useMutation = (options) => options;`],
]) {
  semanticTest(`requires actual imported origins: ${name}`, (t) => {
    const f = fixture(t, owner(`const synchronize = useCallback(async () => { await ${invalidate}; }, [client]);
      useMutation({ mutationFn: save, onSuccess: synchronize });`, header));
    assertUnresolved(f, f.add());
  });
}

semanticTest('does not trust an unknown imported helper by its name', (t) => {
  const f = fixture(t, owner('useMutation({ mutationFn: save, onSuccess: synchronize });',
    `${imports}\nimport { synchronize } from 'unknown-cache-adapter';`));
  assertUnresolved(f, f.add());
});

semanticTest('requires the pinned rule identity, message, location and parseable source', (t) => {
  const source = owner(`${helper} useMutation({ mutationFn: save, onSuccess: synchronize });`);
  const f = fixture(t, source);
  const diagnostic = f.add();
  const original = { ...diagnostic };
  for (const changes of [{ plugin: 'other' }, { rule: 'other-rule' }, { message: 'different claim' },
    { line: 9999 }, { column: 9999 }, { line: 1, column: 1 }]) {
    Object.assign(diagnostic, original, changes);
    assertUnresolved(f, diagnostic);
  }
  Object.assign(diagnostic, original);
  assertUnresolved(f, diagnostic, 'react-doctor@0.2.17');
  f.write(f.file, 'export const invalid = (');
  assertUnresolved(f, diagnostic);
});
