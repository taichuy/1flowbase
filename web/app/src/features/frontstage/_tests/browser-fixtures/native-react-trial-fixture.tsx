import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';

import { JsBlockTrialPanel } from '../../components/JsBlockTrialPanel';
import type { FrontstageBlockInstance } from '../../lib/page-document';

const baseBlock = {
  rendererVersion: 'v1',
  sourceId: 'native-trial',
  codeRef: 'native-trial-code',
  sourceCodeRef: 'native-trial-code',
  catalog: { providerCode: 'official', installationId: 'native-trial' },
  contribution: {
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    code: 'native-react'
  },
  props: {},
  ports: { inputs: [], outputs: [] },
  presentation: { heightMode: 'auto', height: null },
  layout: { order: 0 },
  order: 0,
  runtime: { kind: 'iframe', entry: 'default', hint: 'native-react' }
} satisfies Omit<FrontstageBlockInstance, 'id'>;

const firstSource = `
import { useState } from 'react';
import { Button, Select } from 'antd';
export default function Block() {
  const [count, setCount] = useState(0);
  return <>
    <style>{\`:root { --tone: red; } @keyframes pulse { from { opacity: .8; } to { opacity: 1; } } .same { color: var(--tone); animation: pulse 1s; }\`}</style>
    <div className="same" data-testid="first-native-output">first:{count}</div>
    <Button onClick={() => setCount((value) => value + 1)}>increment-first</Button>
    <Select open value="first" options={[{ value: 'first', label: 'first-popup' }]} />
  </>;
}`;

const secondSource = `
import { Button, Select } from 'antd';
export default function Block() {
  return <>
    <style>{\`:root { --tone: blue; } @keyframes pulse { from { opacity: .6; } to { opacity: 1; } } .same { color: var(--tone); animation: pulse 1s; }\`}</style>
    <div className="same" data-testid="second-native-output">second</div>
    <Button>second-button</Button>
    <Select open value="second" options={[{ value: 'second', label: 'second-popup' }]} />
  </>;
}`;

function NativeReactTrialFixture() {
  const [firstCode, setFirstCode] = useState(firstSource);
  const [firstRevision, setFirstRevision] = useState(1);
  return (
    <main>
      <button
        data-testid="edit-without-run"
        onClick={() => setFirstCode(firstSource.replace('first:', 'edited:'))}
      >
        edit without run
      </button>
      <button
        data-testid="run-edited"
        onClick={() => setFirstRevision((value) => value + 1)}
      >
        run edited
      </button>
      <button
        data-testid="compile-error"
        onClick={() => {
          setFirstCode('export default function Block() { return <div>; }');
          setFirstRevision((value) => value + 1);
        }}
      >
        compile error
      </button>
      <button
        data-testid="render-error"
        onClick={() => {
          setFirstCode(
            "export default function Block() { throw new Error('fixture render error'); }"
          );
          setFirstRevision((value) => value + 1);
        }}
      >
        render error
      </button>
      <section data-testid="native-trial-first">
        <JsBlockTrialPanel
          block={{ ...baseBlock, id: 'native-trial-first' }}
          catalogEntry={null}
          code={firstCode}
          contextSnapshot={{}}
          limits={{ timeoutMs: 1_000 }}
          revision={`run:${firstRevision}`}
        />
      </section>
      <section data-testid="native-trial-second">
        <JsBlockTrialPanel
          block={{ ...baseBlock, id: 'native-trial-second' }}
          catalogEntry={null}
          code={secondSource}
          contextSnapshot={{}}
          limits={{ timeoutMs: 1_000 }}
          revision="run:1"
        />
      </section>
    </main>
  );
}

const root = document.getElementById('root');
if (!root) throw new Error('Native React trial fixture root is missing.');
createRoot(root).render(<NativeReactTrialFixture />);
