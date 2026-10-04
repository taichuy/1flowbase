import {
  act,
  fireEvent,
  render,
  waitFor,
  within
} from '@testing-library/react';
import { App, ConfigProvider } from 'antd';
import { afterEach, expect, test } from 'vitest';
import { Suspense, lazy, type ComponentType } from 'react';
import {
  compileNativeReactComponent,
  validateNativeTrustedBlockSource
} from '@1flowbase/page-runtime';
import { createFrontstageNativeReactModuleRegistry } from '../../registry';
import { createNativeBlockSurfaceRuntime } from '../../surface/native-block-surface-runtime';
import { NativeBlockSurfaceProvider } from '../../native-block-surface-context';
import { evaluateFrontstageReactArtifact } from '../evaluator';

const disposals: Array<() => void> = [];
afterEach(() => {
  disposals.splice(0).forEach((dispose) => dispose());
});

function host(Component: ComponentType, name: string, strict = false) {
  const element = document.createElement('div');
  document.body.append(element);
  const shadow = element.attachShadow({ mode: 'open' });
  const root = document.createElement('div');
  shadow.append(root);
  const overlay = document.createElement('div');
  shadow.append(overlay);
  const runtime = createNativeBlockSurfaceRuntime({
    layoutEpoch: '1',
    scrollOwner: window,
    targetRoot: shadow,
    overlayHost: {
      container: overlay,
      getPopupContainer: () => overlay,
      dispose() {
        overlay.remove();
      }
    }
  });
  const view = render(
    <ConfigProvider theme={{ token: { motion: false } }}>
      <NativeBlockSurfaceProvider scope={runtime}>
        <App>
          <Suspense fallback={<span>loading content</span>}>
            <Component />
          </Suspense>
        </App>
      </NativeBlockSurfaceProvider>
    </ConfigProvider>,
    { container: root, reactStrictMode: strict }
  );
  disposals.push(() => {
    view.unmount();
    runtime.dispose();
    element.remove();
  });
  return { view, runtime, overlay: within(overlay), root: within(root), name };
}
async function compile(source: string) {
  const registry = createFrontstageNativeReactModuleRegistry();
  const compiled = compileNativeReactComponent(source, registry.definitions);
  if (!compiled.ok) throw new Error(JSON.stringify(compiled.diagnostics));
  const evaluated = await evaluateFrontstageReactArtifact(
    compiled.artifact,
    registry
  );
  if (!evaluated.ok) throw new Error(JSON.stringify(evaluated.diagnostics));
  return evaluated.component as ComponentType;
}

test('AC-002 cached factory keeps static modals and destroyAll local to each mount', async () => {
  const Component = await compile(`import {Modal} from 'antd';
export default function Block(){return <><button onClick={()=>Modal.confirm({title:'owned dialog'})}>open</button><button onClick={()=>Modal.destroyAll()}>clear</button></>;}`);
  const a = host(Component, 'a'),
    b = host(Component, 'b');
  fireEvent.click(a.root.getByText('open'));
  fireEvent.click(b.root.getByText('open'));
  await a.overlay.findAllByText('owned dialog');
  await b.overlay.findAllByText('owned dialog');
  fireEvent.click(a.root.getByText('clear'));
  await waitFor(() => expect(a.overlay.queryByRole('dialog')).not.toBeInTheDocument());
  expect(b.overlay.getByRole('dialog')).toBeInTheDocument();
  b.view.unmount();
  expect(b.overlay.queryByRole('dialog')).not.toBeInTheDocument();
});

test('AC-001 preserves holder context, updates and awaited confirmation', async () => {
  const Component =
    await compile(`import React,{createContext,useState} from 'react';import {Modal} from 'antd';
 const Context=createContext('outside');
 export default function Block(){const [modal,holder]=Modal.useModal();const [result,setResult]=useState('pending');return <Context.Provider value="inside"><button onClick={async()=>{const dialog=modal.confirm({title:'before',content:<Context.Consumer>{v=>v}</Context.Consumer>});dialog.update({title:'after'});setResult(String(await dialog));}}>open</button><span>{result}</span>{holder}</Context.Provider>;}`);
  const a = host(Component, 'a');
  fireEvent.click(a.root.getByText('open'));
  await a.overlay.findAllByText('after');
  expect(a.overlay.getByText('inside')).toBeInTheDocument();
  fireEvent.click(a.overlay.getByText('OK'));
  await a.root.findByText('true');
});

test('AC-002 callbacks retained after unmount cannot reopen, even when factory remounts', async () => {
  const callbacks: Array<() => void> = [];
  const registry = createFrontstageNativeReactModuleRegistry({
    '@1flowbase/ui': { retain: (fn: () => void) => callbacks.push(fn) }
  });
  const compiled = compileNativeReactComponent(
    `import {Modal} from 'antd';import {retain} from '@1flowbase/ui';export default function Block(){return <button onClick={()=>retain(()=>Modal.info({title:'late'}))}>retain</button>;}`,
    registry.definitions.map((d) =>
      d.module_source === '@1flowbase/ui'
        ? { ...d, exports: [...d.exports, 'retain'] }
        : d
    )
  );
  if (!compiled.ok) throw Error(JSON.stringify(compiled.diagnostics));
  const evaluated = await evaluateFrontstageReactArtifact(
    compiled.artifact,
    registry
  );
  if (!evaluated.ok) throw Error('evaluation');
  const Component = evaluated.component as ComponentType;
  const a = host(Component, 'a');
  fireEvent.click(a.root.getByText('retain'));
  a.view.unmount();
  const b = host(Component, 'b');
  await act(async () => callbacks[0]());
  expect(b.overlay.queryByRole('dialog')).not.toBeInTheDocument();
  expect(a.overlay.queryByRole('dialog')).not.toBeInTheDocument();
});

test('legacy evaluator retains the raw-global Modal restriction', () => {
  expect(
    validateNativeTrustedBlockSource(
      "import {Modal} from 'antd'; export default function Block(){Modal.confirm({});return null;}"
    ).ok
  ).toBe(false);
});

test('AC-002 lazy modal content survives Suspense hiding and reconnecting the Block', async () => {
  let finish!: (module: { default: ComponentType }) => void;
  const Lazy = lazy(
    () =>
      new Promise<{ default: ComponentType }>((resolve) => {
        finish = resolve;
      })
  );
  const registry = createFrontstageNativeReactModuleRegistry({
    '@1flowbase/ui': { Lazy }
  });
  const compiled = compileNativeReactComponent(
    `import {Modal} from 'antd';import {Lazy} from '@1flowbase/ui';export default function Block(){return <button onClick={()=>Modal.info({title:'lazy modal',content:<Lazy/>})}>open</button>;}`,
    registry.definitions.map((d) =>
      d.module_source === '@1flowbase/ui'
        ? { ...d, exports: [...d.exports, 'Lazy'] }
        : d
    )
  );
  if (!compiled.ok) throw Error(JSON.stringify(compiled.diagnostics));
  const evaluated = await evaluateFrontstageReactArtifact(
    compiled.artifact,
    registry
  );
  if (!evaluated.ok) throw Error('evaluation');
  const a = host(evaluated.component as ComponentType, 'suspense');
  fireEvent.click(a.root.getByText('open'));
  expect(a.root.queryByText('loading content')).not.toBeInTheDocument();
  fireEvent.click(a.root.getByText('open'));
  await act(async () => finish({ default: () => <span>loaded content</span> }));
  await waitFor(() =>
    expect(a.overlay.getAllByText('loaded content')).toHaveLength(2)
  );
  expect(a.overlay.getAllByRole('dialog')).toHaveLength(2);
  a.view.unmount();
  expect(a.overlay.queryByRole('dialog')).not.toBeInTheDocument();
});

test('AC-002 destroy settles confirmation and layout invalidation releases handles', async () => {
  const Component =
    await compile(`import {useState} from 'react'; import {Modal} from 'antd';
 export default function Block(){const [value,setValue]=useState('pending');return <><button onClick={async()=>setValue(String(await Modal.confirm({title:'waiting'})))}>open</button><button onClick={()=>Modal.destroyAll()}>clear</button><span>{value}</span></>;}`);
  const a = host(Component, 'a');
  fireEvent.click(a.root.getByText('open'));
  await a.overlay.findByRole('dialog');
  fireEvent.click(a.root.getByText('clear'));
  await a.root.findByText('false');
  fireEvent.click(a.root.getByText('open'));
  await a.overlay.findByRole('dialog');
  await act(async () => {
    a.runtime.advanceLayoutEpoch('2');
  });
  await waitFor(() => expect(a.overlay.queryByRole('dialog')).not.toBeInTheDocument());
});

test('AC-001 allows computed access and opens from a layout effect', async () => {
  const Component =
    await compile(`import {useLayoutEffect} from 'react'; import {Modal} from 'antd';
 export default function Block(){useLayoutEffect(()=>{const h=Modal['confirm']({title:'layout'});return ()=>h.destroy();},[]);return null;}`);
  const a = host(Component, 'a');
  await a.overlay.findByRole('dialog');
});

test('AC-003 resolves difference and draggable through the registered modules', async () => {
  const Component =
    await compile(`import difference from 'lodash/difference'; import Draggable,{DraggableCore} from 'react-draggable';
 export default function Block(){return <span>{difference([1,2,3],[2]).join(',')+' '+typeof Draggable+' '+typeof DraggableCore}</span>;}`);
  const a = host(Component, 'a');
  expect(a.root.getByText('1,3 function function')).toBeInTheDocument();
});

test('AC-001 StrictMode preserves async onOk and cancellation semantics', async () => {
  const Component =
    await compile(`import {useState} from 'react'; import {Modal} from 'antd';
 export default function Block(){const [value,setValue]=useState('pending');return <><button onClick={async()=>setValue(String(await Modal.confirm({title:'async',onOk:()=>new Promise(resolve=>setTimeout(resolve,20))})))}>open</button><span>{value}</span></>;}`);
  const a = host(Component, 'strict', true);
  fireEvent.click(a.root.getByText('open'));
  await a.overlay.findByRole('dialog');
  fireEvent.click(a.overlay.getByText('OK'));
  expect(a.root.getByText('pending')).toBeInTheDocument();
  await a.root.findByText('true');
  await waitFor(() => expect(a.overlay.queryByRole('dialog')).not.toBeInTheDocument());
  fireEvent.click(a.root.getByText('open'));
  await a.overlay.findByRole('dialog');
  fireEvent.click(a.overlay.getByText('Cancel'));
  await a.root.findByText('false');
});
