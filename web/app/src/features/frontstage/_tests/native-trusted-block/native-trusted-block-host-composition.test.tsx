import { render, screen, waitFor } from '@testing-library/react';
import { readdirSync, readFileSync } from 'node:fs';
import { extname, join } from 'node:path';
import type { ReactNode } from 'react';
import { describe, expect, test, vi } from 'vitest';

import {
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME,
  createNativeTrustedBlockHost,
  prepareNativeTrustedBlock,
  type NativeTrustedBlockPrepareInput
} from '@1flowbase/page-runtime';
import type { BlockContext } from '@1flowbase/page-protocol';

import {
  createFrontstageNativeTrustedBlockReactAdapter,
  type FrontstageNativeTrustedBlockCreateRoot
} from '../../lib/native-trusted-block-react-adapter';
import { createFrontstageNativeTrustedBlockRuntimeFactory } from '../../lib/native-trusted-block-runtime-factory';

const NATIVE_STYLE_SCOPE_ROOT_ATTRIBUTE =
  'data-flowbase-native-trusted-block-root';
const NATIVE_STYLE_SCOPE_ID_ATTRIBUTE = 'data-flowbase-native-trusted-block-id';

function createTestingRoot(): {
  createRoot: FrontstageNativeTrustedBlockCreateRoot;
  renderSpy: ReturnType<typeof vi.fn>;
  unmountSpy: ReturnType<typeof vi.fn>;
} {
  const renderSpy = vi.fn();
  const unmountSpy = vi.fn();
  let unmountRendered: (() => void) | undefined;

  return {
    renderSpy,
    unmountSpy,
    createRoot(root) {
      return {
        render(children: ReactNode) {
          renderSpy();
          unmountRendered = render(<>{children}</>, {
            container: root as HTMLElement
          }).unmount;
        },
        unmount() {
          unmountSpy();
          unmountRendered?.();
          unmountRendered = undefined;
        }
      };
    }
  };
}

function createBlockRoot(): HTMLDivElement {
  const root = document.createElement('div');
  document.body.append(root);
  return root;
}

function createFakeBlockContext(
  overrides: Partial<BlockContext> = {}
): BlockContext {
  return {
    currentUser: null,
    workspace: { id: 'workspace-1', name: 'Workspace' },
    application: { id: 'application-1', name: 'Application' },
    page: { id: 'page-1', route: '/page-1', title: 'Page' },
    params: {},
    props: {},
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
    events: {
      emit: vi.fn()
    },
    theme: { mode: 'light', tokens: {} },
    ui: {},
    ...overrides,
    inputs: overrides.inputs ?? {}
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
    <Space title="native-composition-block">
      <Button type="primary">{props.props.title}</Button>
      <Button>{props.ctx.props.title}</Button>
      <Button>
        {String(props.portalContainment.root instanceof HTMLElement)}
      </Button>
    </Space>
  );
}
`,
    props: { title: 'Prepared JSX AntD block' },
    actorPermissions: [NATIVE_TRUSTED_BLOCK_PERMISSION],
    ...overrides
  };
}

describe('native trusted block host composition smoke contract', () => {
  test('mounts a prepared permissioned JSX source through host, runtime factory, and React adapter', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const query = vi.fn(async () => ({ title: 'Resolved by controlled ctx' }));
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      providerWrapper: (children) => (
        <div title="native-provider-scope">{children}</div>
      ),
      resolveBlockContext: () =>
        createFakeBlockContext({
          props: { title: 'Controlled ctx title' },
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
        }),
      resolveComponent: createFrontstageNativeTrustedBlockRuntimeFactory()
    });
    const host = createNativeTrustedBlockHost({ adapter });
    const prepareResult = prepareNativeTrustedBlock(createPrepareInput());

    expect(prepareResult.ok).toBe(true);
    if (!prepareResult.ok) {
      throw new Error('Expected native trusted block prepare to succeed.');
    }

    const mountedState = await host.mount(prepareResult.plan, root);

    expect(mountedState).toMatchObject({
      status: 'mounted',
      blockId: 'host-composition-native-block',
      runtime: NATIVE_TRUSTED_BLOCK_RUNTIME
    });
    expect(testingRoot.renderSpy).toHaveBeenCalledTimes(1);
    expect(
      await screen.findByRole('button', { name: 'Prepared JSX AntD block' })
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('button', { name: 'Controlled ctx title' })
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('button', { name: 'true' })
    ).toBeInTheDocument();
    expect(query).toHaveBeenCalledWith('/api/console/test', {
      body: { title: 'Prepared JSX AntD block' }
    });
    expect(root).toHaveAttribute(NATIVE_STYLE_SCOPE_ROOT_ATTRIBUTE, '');
    expect(root).toHaveAttribute(
      NATIVE_STYLE_SCOPE_ID_ATTRIBUTE,
      'host-composition-native-block'
    );
    expect(
      await screen.findByTitle('native-provider-scope')
    ).toBeInTheDocument();

    const disposedState = await host.dispose();

    expect(disposedState).toEqual({ status: 'disposed' });
    expect(testingRoot.unmountSpy).toHaveBeenCalledTimes(1);
    expect(root).not.toHaveAttribute(NATIVE_STYLE_SCOPE_ROOT_ATTRIBUTE);
    expect(root).not.toHaveAttribute(NATIVE_STYLE_SCOPE_ID_ATTRIBUTE);
    await waitFor(() => {
      expect(
        screen.queryByRole('button', { name: 'Prepared JSX AntD block' })
      ).not.toBeInTheDocument();
    });
  });

  test('does not enter React mount when prepare rejects missing native permission', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: createFrontstageNativeTrustedBlockRuntimeFactory()
    });
    const host = createNativeTrustedBlockHost({ adapter });
    const prepareResult = prepareNativeTrustedBlock(
      createPrepareInput({ actorPermissions: ['workspace.read'] })
    );

    expect(prepareResult.ok).toBe(false);
    expect(prepareResult.errors[0]).toMatchObject({
      code: 'action_denied',
      path: 'actorPermissions'
    });
    if (prepareResult.ok) {
      await host.mount(prepareResult.plan, root);
    }

    expect(host.getState()).toEqual({ status: 'idle' });
    expect(testingRoot.renderSpy).not.toHaveBeenCalled();
    expect(testingRoot.unmountSpy).not.toHaveBeenCalled();
    expect(root).not.toHaveAttribute(NATIVE_STYLE_SCOPE_ROOT_ATTRIBUTE);
  });

  test('reports render-time capability guard violations through the host adapter boundary', async () => {
    const root = createBlockRoot();
    const onRuntimeError = vi.fn();
    const consoleErrorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      onRuntimeError,
      resolveComponent: createFrontstageNativeTrustedBlockRuntimeFactory()
    });
    const host = createNativeTrustedBlockHost({ adapter });
    const prepareResult = prepareNativeTrustedBlock(
      createPrepareInput({
        source: `
import React from 'react';
import { Button } from 'antd';

export default function CapabilityViolationBlock() {
  f\\u0065tch('/api/native-trusted-block');
  return <Button>Denied</Button>;
}
`
      })
    );

    try {
      expect(prepareResult.ok).toBe(true);
      if (!prepareResult.ok) {
        throw new Error(
          'Expected capability violation source prepare to succeed.'
        );
      }

      const mountedState = await host.mount(prepareResult.plan, root);

      expect(mountedState.status).toBe('mounted');
      expect(testingRoot.renderSpy).toHaveBeenCalledTimes(1);
      await waitFor(() => {
        expect(onRuntimeError).toHaveBeenCalledWith(
          expect.objectContaining({
            code: 'runtime_error',
            path: 'runtime.capability.fetch'
          }),
          expect.objectContaining({
            blockId: 'host-composition-native-block',
            root,
            plan: expect.objectContaining({
              blockId: 'host-composition-native-block'
            })
          })
        );
      });
      expect(root).not.toHaveTextContent('runtime.capability.fetch');
    } finally {
      consoleErrorSpy.mockRestore();
      await host.dispose();
    }
  });

  test('is not statically imported by existing FrontStage UI, catalog, route, or app code', () => {
    const frontstageDir = join(process.cwd(), 'src/features/frontstage');
    const matches = collectSourceFiles([
      join(frontstageDir, 'pages'),
      join(frontstageDir, 'components'),
      join(frontstageDir, 'api'),
      join(frontstageDir, 'hooks'),
      join(process.cwd(), 'src/routes'),
      join(process.cwd(), 'src/app')
    ])
      .concat(
        collectSourceFiles([join(frontstageDir, 'lib')]).filter((filePath) =>
          filePath.includes('block-catalog')
        )
      )
      .filter((filePath) =>
        [
          'native-trusted-block-react-adapter',
          'native-trusted-block-runtime-factory',
          'createNativeTrustedBlockHost',
          'prepareNativeTrustedBlock'
        ].some((marker) => readFileSync(filePath, 'utf8').includes(marker))
      );

    expect(matches).toEqual([]);
  });
});

function collectSourceFiles(directories: string[]): string[] {
  const files: string[] = [];

  for (const directory of directories) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const entryPath = join(directory, entry.name);
      if (entry.isDirectory()) {
        files.push(...collectSourceFiles([entryPath]));
        continue;
      }

      if (['.ts', '.tsx'].includes(extname(entry.name))) {
        files.push(entryPath);
      }
    }
  }

  return files;
}
