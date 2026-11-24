import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { App, ConfigProvider } from 'antd';
import { Suspense, type ComponentType } from 'react';
import { afterEach, expect, test } from 'vitest';
import { compileNativeReactComponent } from '@1flowbase/page-runtime';

import { evaluateFrontstageReactArtifact } from '../../modal/evaluator';
import { NativeBlockSurfaceProvider } from '../../native-block-surface-context';
import { createFrontstageNativeReactModuleRegistry } from '../../registry';
import { createNativeBlockSurfaceRuntime } from '../../surface/native-block-surface-runtime';

const disposals: Array<() => void> = [];
afterEach(() => {
  disposals.splice(0).forEach((dispose) => dispose());
});

function mountShadowSurface(Component: ComponentType) {
  const element = document.createElement('div');
  document.body.append(element);
  const shadow = element.attachShadow({ mode: 'open' });
  const root = document.createElement('div');
  shadow.append(root);
  const overlay = document.createElement('div');
  overlay.dataset.testid = 'shadow-overlay-layer';
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
          <Suspense fallback={<span>loading</span>}>
            <Component />
          </Suspense>
        </App>
      </NativeBlockSurfaceProvider>
    </ConfigProvider>,
    { container: root }
  );
  disposals.push(() => {
    view.unmount();
    runtime.dispose();
    element.remove();
  });
  return {
    overlay,
    root,
    overlayScope: within(overlay),
    rootScope: within(root)
  };
}

async function compileBlock(source: string) {
  const registry = createFrontstageNativeReactModuleRegistry();
  const compiled = compileNativeReactComponent(source, registry.definitions);
  if (!compiled.ok) {
    throw new Error(JSON.stringify(compiled.diagnostics));
  }
  const evaluated = await evaluateFrontstageReactArtifact(
    compiled.artifact,
    registry
  );
  if (!evaluated.ok) {
    throw new Error(JSON.stringify(evaluated.diagnostics));
  }
  return evaluated.component as ComponentType;
}

test('mounts Drawer and Popconfirm from antd inside ShadowRoot overlay layer instead of document.body', async () => {
  const Component = await compileBlock(`
    import React, { useState } from 'react';
    import { Button, Drawer, Popconfirm } from 'antd';
    export default function Block() {
      const [open, setOpen] = useState(false);
      const [confirmed, setConfirmed] = useState(false);
      return (
        <div>
          <Button onClick={() => setOpen(true)}>管理员</Button>
          <Popconfirm
            title="确认停用该工作空间？"
            okText="确认停用"
            cancelText="取消"
            onConfirm={() => setConfirmed(true)}
          >
            <Button danger>停用</Button>
          </Popconfirm>
          {confirmed ? <span>已确认停用</span> : null}
          <Drawer title="空间管理员" open={open} onClose={() => setOpen(false)}>
            <div>抽屉内容在ShadowRoot内</div>
          </Drawer>
        </div>
      );
    }
  `);

  const surface = mountShadowSurface(Component);

  fireEvent.click(surface.rootScope.getByRole('button', { name: '管理员' }));
  await surface.overlayScope.findByText('抽屉内容在ShadowRoot内');
  expect(document.body.querySelector('.ant-drawer')).toBeNull();

  fireEvent.click(surface.rootScope.getByRole('button', { name: /停\s*用/u }));
  await surface.overlayScope.findByText('确认停用该工作空间？');
  expect(document.body.querySelector('.ant-popover')).toBeNull();
  fireEvent.click(
    surface.overlayScope.getByRole('button', { name: /确认停用/u })
  );
  await surface.rootScope.findByText('已确认停用');
});

test('preserves inline Drawer getContainer={false} and scopes antd/es subpath imports', async () => {
  const Component = await compileBlock(`
    import React, { useState } from 'react';
    import Drawer from 'antd/es/drawer';
    import Popconfirm from 'antd/es/popconfirm';
    export default function Block() {
      const [inlineOpen, setInlineOpen] = useState(false);
      return (
        <div data-testid="inline-host">
          <button onClick={() => setInlineOpen(true)}>open-inline</button>
          <Popconfirm title="subpath-popconfirm" okText="ok" cancelText="cancel">
            <button>open-pop</button>
          </Popconfirm>
          <Drawer title="inline-drawer" open={inlineOpen} getContainer={false}>
            <span>inline-drawer-body</span>
          </Drawer>
        </div>
      );
    }
  `);

  const surface = mountShadowSurface(Component);
  fireEvent.click(surface.rootScope.getByText('open-inline'));
  await surface.rootScope.findByText('inline-drawer-body');
  expect(surface.overlay.querySelector('.ant-drawer')).toBeNull();

  fireEvent.click(surface.rootScope.getByText('open-pop'));
  await waitFor(() =>
    expect(
      surface.overlayScope.getByText('subpath-popconfirm')
    ).toBeInTheDocument()
  );
});
