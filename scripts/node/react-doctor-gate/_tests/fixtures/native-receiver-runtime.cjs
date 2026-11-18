const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { createHash } = require('node:crypto');
const { createRequire } = require('node:module');
const { spawnSync } = require('node:child_process');

// The same receiver threats apply to QueryClient and AntD Form. No fixture path,
// application key or domain method is used to authorize the production proof.
function receiverThreats(receiver, method, port, replacement = 'submit') {
  const property = `${method}: ${replacement}`;
  return [
    ['QA Object.assign replacement', `Object.assign(${receiver}, { ${property} });`],
    ['QA defineProperty replacement', `Object.defineProperty(${receiver}, '${method}', { value: ${replacement} });`],
    ['QA opaque receiver escape', `${port}(${receiver});`],
    ['immutable receiver alias', `const alias = ${receiver}; Object.assign(alias, { ${property} });`],
    ['nested immutable aliases', `const first = ${receiver}; const second = first; Object.defineProperty(second, '${method}', { value: submit });`],
    ['destructured object receiver alias', `const { current: alias } = { current: ${receiver} }; ${port}(alias);`],
    ['destructured array receiver alias', `const [alias] = [${receiver}]; ${port}(alias);`],
    ['nested container escape', `patchContainer({ nested: { current: ${receiver} } });`],
    ['container retained by opaque port', `const box = { current: ${receiver} }; patchContainer(box);`],
    ['receiver returned through callback', `retainReceiver(() => ${receiver});`],
    ['defineProperties replacement', `Object.defineProperties(${receiver}, { ${method}: { value: submit } });`],
    ['Reflect.set replacement', `Reflect.set(${receiver}, '${method}', submit);`],
    ['aliased descriptor function', `const replace = Object.defineProperty; replace(${receiver}, '${method}', { value: submit });`],
    ['destructured reflection function', `const { set: replace } = Reflect; replace(${receiver}, '${method}', submit);`],
    ['prototype replacement', `delete ${receiver}.${method}; Object.setPrototypeOf(${receiver}, { ${method}: submit });`],
    ['__proto__ replacement', `delete ${receiver}.${method}; ${receiver}.__proto__ = { ${method}: submit };`],
    ['computed descriptor key', `const method = '${method}'; Object.defineProperty(${receiver}, method, { value: submit });`],
    ['computed direct method write', `const method = '${method}'; ${receiver}[method] = submit;`],
    ['deleted native method', `delete ${receiver}.${method};`],
    ['shadowed Object readonly-looking method', `const Object = { keys: ${port} }; Object.keys(${receiver});`],
    ['shadowed Object mutator', `const Object = { assign: ${port} }; Object.assign(${receiver}, { ${method}: submit });`],
    ['local opaque helper receives receiver', `const delegate = value => patchContainer({ current: value }); delegate(${receiver});`],
  ];
}

function nativeReceivers(kind, cases) {
  const result = spawnSync(process.execPath, [__filename], {
    input: JSON.stringify({ kind, cases }), encoding: 'utf8',
    timeout: 15_000, maxBuffer: 256 * 1024,
  });
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(result.signal, null, `native receiver worker terminated: ${result.stderr}`);
  assert.equal(result.status, 0, `native receiver worker failed: ${result.stderr}`);
  const observations = JSON.parse(result.stdout);
  assert.equal(observations.length, cases.length, 'every native control must actually execute');
  return observations;
}

async function observe({ kind, cases }) {
  const appRequire = createRequire(path.resolve(__dirname, '../../../../../web/app/package.json'));
  const results = [];
  const QueryClient = kind === 'query' ? appRequire('@tanstack/react-query').QueryClient : null;
  const formRequire = kind === 'form' ? createRequire(appRequire.resolve('antd/package.json')) : null;
  const FormStore = formRequire ? formRequire('@rc-component/form/lib/hooks/useForm').FormStore : null;
  for (const { name, source } of cases) {
    let businessSubmits = 0;
    const submit = () => { businessSubmits++; };
    const effects = [];
    const store = FormStore ? new FormStore(() => {}) : null;
    const native = QueryClient
      ? new QueryClient({ defaultOptions: { queries: { gcTime: Infinity, retry: false } } })
      : store.getForm();
    if (QueryClient) native.setQueryData(['records'], [{ id: 1 }]);
    else native.setFieldsValue({ value: 0 });
    const before = QueryClient ? native.getQueryState(['records']).isInvalidated : native.getFieldValue('value');
    const context = {
      useQueryClient: () => native, useMutation: (options) => options,
      Form: { useForm: () => [native] }, useEffect: (callback) => effects.push(callback),
      save() {}, submit,
      patchClient: (client) => Object.assign(client, { invalidateQueries() {} }),
      patchForm: (form) => Object.assign(form, { setFieldsValue: submit }),
    };
    try {
      const invocation = QueryClient ? '\nOwner().onSuccess();' : '\nOwner({ receipt: { value: 1 } });';
      await vm.runInNewContext(source.replace(/^import[^\n]*\n/gm, '') + invocation, context, { timeout: 1000 });
      for (const effect of effects) await effect();
      results.push({
        name, sourceSha256: createHash('sha256').update(source).digest('hex'), before, businessSubmits,
        library: QueryClient ? appRequire.resolve('@tanstack/react-query') : formRequire.resolve('@rc-component/form/lib/hooks/useForm'),
        value: QueryClient ? native.getQueryState(['records']).isInvalidated : native.getFieldValue('value'),
      });
    } finally {
      if (QueryClient) native.clear();
      else {
        if (store.timeoutId) clearTimeout(store.timeoutId);
        store.destroyForm(true);
      }
    }
  }
  return results;
}

if (require.main === module) {
  // Exit only this bounded worker, after all real library observations and
  // cleanup. Never terminate the parent test runner or skip pending test cases.
  observe(JSON.parse(fs.readFileSync(0, 'utf8'))).then(
    (results) => process.stdout.write(`${JSON.stringify(results)}\n`, () => process.exit(0)),
    (error) => { process.stderr.write(`${error.stack}\n`); process.exit(1); },
  );
}

module.exports = { receiverThreats, nativeReceivers };
