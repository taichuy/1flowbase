import { render, waitFor, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type { BlockContext } from '@1flowbase/page-protocol';
import {
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME,
  prepareNativeTrustedBlock,
  type NativeTrustedBlockPrepareInput
} from '@1flowbase/page-runtime';

import { FrontstageNativeTrustedBlockPortalHost } from '../../lib/native-trusted-block-react-adapter';
import { createFrontstageNativeTrustedBlockRuntimeFactory } from '../../lib/native-trusted-block-runtime-factory';

function createContext(overrides: Partial<BlockContext> = {}): BlockContext {
  return {
    currentUser: null,
    workspace: { id: 'workspace-1' },
    application: null,
    page: { id: 'page-1', route: '/page-1' },
    inputs: {},
    outputs: { publish: vi.fn() },
    params: {},
    props: { title: 'Controlled ctx title' },
    state: {},
    patch: vi.fn(),
    api: {
      get: vi.fn(),
      post: vi.fn(),
      put: vi.fn(),
      patch: vi.fn(),
      delete: vi.fn(),
      head: vi.fn(),
      options: vi.fn(),
      stream: vi.fn()
    },
    events: { emit: vi.fn() },
    theme: { mode: 'light', tokens: {} },
    ui: {},
    ...overrides
  };
}

function createPrepareInput(
  overrides: Partial<NativeTrustedBlockPrepareInput> = {}
): NativeTrustedBlockPrepareInput {
  return {
    runtime: NATIVE_TRUSTED_BLOCK_RUNTIME,
    blockId: 'host-composition-native-block',
    entry: './HostCompositionBlock.tsx',
    source: `
import React from 'react';
import { Button, Space } from 'antd';

export default function HostCompositionBlock(props) {
  void props.ctx.api.get('/api/console/test', { body: { title: props.props.title } });
  return (
    <Space>
      <Button>{props.props.title}</Button>
      <Button>{props.ctx.props.title}</Button>
      <Button>{String(props.portalContainment.root instanceof ShadowRoot)}</Button>
    </Space>
  );
}
`,
    props: { title: 'Prepared JSX AntD block' },
    actorPermissions: [NATIVE_TRUSTED_BLOCK_PERMISSION],
    ...overrides
  };
}

function createBlockRoot(): HTMLDivElement {
  const root = document.createElement('div');
  document.body.append(root);
  return root;
}

function shadowQueries(root: Element) {
  if (!root.shadowRoot) throw new Error('Expected a native block ShadowRoot.');
  return within(root.shadowRoot as unknown as HTMLElement);
}

describe('native trusted block surface portal composition', () => {
  test('D3R-AC-001 composes prepare, component factory, controlled context, and portal host', async () => {
    const root = createBlockRoot();
    const query = vi.fn(async () => ({ title: 'Resolved' }));
    const result = prepareNativeTrustedBlock(createPrepareInput());
    expect(result.ok).toBe(true);
    if (!result.ok) throw new Error('Expected native prepare to succeed.');
    const component =
      createFrontstageNativeTrustedBlockRuntimeFactory()(result.plan);
    const context = createContext({
      api: {
        get: query as BlockContext['api']['get'],
        post: vi.fn(),
        put: vi.fn(),
        patch: vi.fn(),
        delete: vi.fn(),
        head: vi.fn(),
        options: vi.fn(),
        stream: vi.fn()
      }
    });

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="composition:1"
        plan={result.plan}
        component={component}
        ctx={context}
        providerWrapper={(children) => (
          <section title="native-provider-scope">{children}</section>
        )}
      />
    );

    expect(
      await shadowQueries(root).findByRole('button', {
        name: 'Prepared JSX AntD block'
      })
    ).toBeInTheDocument();
    expect(
      await shadowQueries(root).findByRole('button', {
        name: 'Controlled ctx title'
      })
    ).toBeInTheDocument();
    expect(
      await shadowQueries(root).findByRole('button', { name: 'true' })
    ).toBeInTheDocument();
    expect(query).toHaveBeenCalledWith('/api/console/test', {
      body: { title: 'Prepared JSX AntD block' }
    });
    expect(
      await shadowQueries(root).findByTitle('native-provider-scope')
    ).toBeInTheDocument();
  });

  test('permission rejection never creates a portal surface', () => {
    const root = createBlockRoot();
    const result = prepareNativeTrustedBlock(
      createPrepareInput({ actorPermissions: ['workspace.read'] })
    );

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'action_denied',
      path: 'actorPermissions'
    });
    expect(root.shadowRoot).toBeNull();
  });

  test('D3R-AC-007 reports capability render failure without leaking details into the surface', async () => {
    const root = createBlockRoot();
    const onRuntimeError = vi.fn();
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const result = prepareNativeTrustedBlock(
      createPrepareInput({
        source: `
import React from 'react';
export default function CapabilityViolationBlock() {
  f\\u0065tch('/api/native-trusted-block');
  return <div>Denied</div>;
}
`
      })
    );

    try {
      expect(result.ok).toBe(true);
      if (!result.ok) throw new Error('Expected prepare to succeed.');
      render(
        <FrontstageNativeTrustedBlockPortalHost
          root={root}
          renderEpoch="capability:1"
          plan={result.plan}
          component={
            createFrontstageNativeTrustedBlockRuntimeFactory()(result.plan)
          }
          ctx={createContext()}
          onRuntimeError={onRuntimeError}
        />
      );

      await waitFor(() =>
        expect(onRuntimeError).toHaveBeenCalledWith(
          expect.objectContaining({
            code: 'runtime_error',
            path: 'runtime.capability.fetch'
          }),
          expect.objectContaining({
            blockId: 'host-composition-native-block',
            root
          })
        )
      );
      expect(root.shadowRoot).not.toHaveTextContent('runtime.capability.fetch');
    } finally {
      consoleError.mockRestore();
    }
  });
});
