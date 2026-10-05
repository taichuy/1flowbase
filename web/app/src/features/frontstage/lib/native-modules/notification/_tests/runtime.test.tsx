import {
  act,
  fireEvent,
  render,
  waitFor,
  within
} from '@testing-library/react';
import { App, ConfigProvider } from 'antd';
import { afterEach, expect, test } from 'vitest';
import { Suspense, type ComponentType } from 'react';
import {
  compileNativeReactComponent,
  validateNativeTrustedBlockSource
} from '@1flowbase/page-runtime';
import { createFrontstageNativeReactModuleRegistry } from '../../registry';
import { createNativeBlockSurfaceRuntime } from '../../surface/native-block-surface-runtime';
import { NativeBlockSurfaceProvider } from '../../native-block-surface-context';
import { evaluateFrontstageReactArtifact } from '../../modal/evaluator';

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

test('static notification keys, defaults and destroy remain local to each mount', async () => {
  const Component = await compile(
    `import {notification} from 'antd'; export default function Block(){return <><button onClick={()=>{notification.config({duration:0,placement:'bottomLeft'});notification.open({key:'same',title:'first'});}}>open</button><button onClick={()=>notification.open({key:'same',title:'first',duration:0})}>plain</button><button onClick={()=>notification.success({key:'same',title:'updated'})}>update</button><button onClick={()=>notification.destroy('same')}>close</button></>;}`
  );
  const a = host(Component, 'a'),
    b = host(Component, 'b');
  fireEvent.click(a.root.getByText('open'));
  fireEvent.click(b.root.getByText('plain'));
  await a.overlay.findByText('first');
  await b.overlay.findByText('first');
  expect(
    a.overlay.getByText('first').closest('[class*=notification-bottomLeft]')
  ).toBeInTheDocument();
  expect(
    b.overlay.getByText('first').closest('[class*=notification-topRight]')
  ).toBeInTheDocument();
  fireEvent.click(a.root.getByText('update'));
  await a.overlay.findByText('updated');
  expect(b.overlay.getByText('first')).toBeInTheDocument();
  fireEvent.click(a.root.getByText('close'));
  await waitFor(() => expect(a.overlay.queryByText('updated')).not.toBeInTheDocument());
  expect(b.overlay.getByText('first')).toBeInTheDocument();
  b.view.unmount();
  expect(b.overlay.queryByText('first')).not.toBeInTheDocument();
});

test('retained callbacks cannot notify after unmount and defaults do not leak', async () => {
  const Component = await compile(
    `import {notification} from 'antd';export default function Block(){return <><button onClick={()=>{notification.config({duration:0});notification.open({title:'persistent'});}}>configured</button><button onClick={()=>{setTimeout(()=>notification.open({title:'late'}),30);}}>delay</button><button onClick={()=>notification.destroy()}>clear</button></>;}`
  );
  const a = host(Component, 'a');
  fireEvent.click(a.root.getByText('delay'));
  a.view.unmount();
  const b = host(Component, 'b');
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 60));
  });
  expect(b.overlay.queryByText('late')).not.toBeInTheDocument();
  expect(a.overlay.queryByText('late')).not.toBeInTheDocument();
  fireEvent.click(b.root.getByText('configured'));
  await b.overlay.findByText('persistent');
  fireEvent.click(b.root.getByText('clear'));
  await waitFor(() => expect(b.overlay.queryByText('persistent')).not.toBeInTheDocument());
  fireEvent.click(b.root.getByText('configured'));
  await b.overlay.findByText('persistent');
  await act(async () => b.runtime.advanceLayoutEpoch('next'));
  await waitFor(() => expect(b.overlay.queryByText('persistent')).not.toBeInTheDocument());
});

test('legacy source retains static notification restriction', () => {
  expect(
    validateNativeTrustedBlockSource(
      "import {notification} from 'antd';export default ()=>notification.open({});"
    ).ok
  ).toBe(false);
});
