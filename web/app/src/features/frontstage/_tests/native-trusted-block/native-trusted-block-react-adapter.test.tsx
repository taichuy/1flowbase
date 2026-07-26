import { render, waitFor, within } from '@testing-library/react';
import { readdirSync, readFileSync } from 'node:fs';
import { extname, join } from 'node:path';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import type { BlockContext } from '@1flowbase/page-protocol';

import {
  createFrontstageNativeTrustedBlockReactAdapter,
  type FrontstageNativeTrustedBlockCreateRoot
} from '../../lib/native-trusted-block-react-adapter';

const antdProviderRecords = vi.hoisted(() => ({
  configProviderProps: [] as Array<Record<string, unknown>>,
  appRenderCount: 0,
  styleProviderProps: [] as Array<Record<string, unknown>>
}));

vi.mock('@ant-design/cssinjs', async () => {
  const React = await vi.importActual<typeof import('react')>('react');

  return {
    createCache: vi.fn(() => ({})),
    StyleProvider({
      children,
      ...props
    }: {
      children?: ReactNode;
      [key: string]: unknown;
    }) {
      antdProviderRecords.styleProviderProps.push(props);
      return React.createElement(React.Fragment, null, children);
    }
  };
});

vi.mock('antd', async () => {
  const React = await vi.importActual<typeof import('react')>('react');

  return {
    ConfigProvider({
      children,
      ...props
    }: {
      children?: ReactNode;
      [key: string]: unknown;
    }) {
      antdProviderRecords.configProviderProps.push(props);

      return React.createElement(
        'div',
        { 'data-testid': 'mock-config-provider' },
        children
      );
    },
    App({ children }: { children?: ReactNode }) {
      antdProviderRecords.appRenderCount += 1;

      return React.createElement(
        'div',
        { 'data-testid': 'mock-antd-app' },
        children
      );
    }
  };
});

function createPlan(
  overrides: Partial<NativeTrustedBlockPreparePlan> = {}
): NativeTrustedBlockPreparePlan {
  return {
    runtime: 'native_trusted_block',
    blockId: 'native-block-1',
    entry: 'default',
    source: 'export default function Block() { return null; }',
    normalizedSource: 'export default function Block() { return null; }',
    props: { title: 'Quarterly plan', count: 3 },
    requiredPermissions: ['ui_block.javascript.native'],
    ...overrides
  };
}

function createTestingRoot(): {
  createRoot: FrontstageNativeTrustedBlockCreateRoot;
  unmountSpy: ReturnType<typeof vi.fn>;
  roots: Element[];
} {
  const unmountSpy = vi.fn();
  const roots: Element[] = [];
  let unmountRendered: (() => void) | undefined;

  return {
    roots,
    unmountSpy,
    createRoot(root) {
      roots.push(root);

      return {
        render(children: ReactNode) {
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

function getShadowRoot(root: Element): ShadowRoot {
  const shadowRoot = root.shadowRoot;
  if (!shadowRoot) throw new Error('Expected an open native block ShadowRoot.');
  return shadowRoot;
}

function shadowQueries(root: Element) {
  return within(getShadowRoot(root) as unknown as HTMLElement);
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

describe('frontstage native trusted block React adapter', () => {
  beforeEach(() => {
    antdProviderRecords.configProviderProps = [];
    antdProviderRecords.appRenderCount = 0;
    antdProviderRecords.styleProviderProps = [];
  });

  test('creates a React root and renders the resolved native component', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const resolvedComponent = vi.fn(() => (
      <div data-testid="native-block">Ready</div>
    ));
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => resolvedComponent
    });

    await adapter.mount({ plan: createPlan(), root });

    expect(testingRoot.roots).toHaveLength(1);
    expect(testingRoot.roots[0].getRootNode()).toBe(getShadowRoot(root));
    expect(
      await shadowQueries(root).findByTestId('native-block')
    ).toHaveTextContent('Ready');
    expect(resolvedComponent).toHaveBeenCalledTimes(1);
  });

  test('scopes the default AntD popup container to the current block root', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => () => <div data-testid="native-block">Ready</div>
    });

    await adapter.mount({ plan: createPlan(), root });

    const providerProps = antdProviderRecords.configProviderProps[0];
    expect(providerProps).toEqual(
      expect.objectContaining({ getPopupContainer: expect.any(Function) })
    );
    expect((providerProps.getPopupContainer as () => ShadowRoot)()).toBe(
      getShadowRoot(root)
    );
    expect((providerProps.getTargetContainer as () => ShadowRoot)()).toBe(
      getShadowRoot(root)
    );
    expect(antdProviderRecords.styleProviderProps).toEqual([
      expect.objectContaining({
        autoClear: true,
        cache: expect.anything(),
        container: getShadowRoot(root)
      })
    ]);
    expect(antdProviderRecords.appRenderCount).toBe(1);
  });

  test('marks the mounted block root with a stable native trusted style scope', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => () => <div data-testid="native-block">Ready</div>
    });

    await adapter.mount({
      plan: createPlan({ blockId: 'native-block-scoped' }),
      root
    });

    expect(root).toHaveAttribute('data-flowbase-native-trusted-block-root', '');
    expect(root).toHaveAttribute(
      'data-flowbase-native-trusted-block-id',
      'native-block-scoped'
    );
  });

  test('keeps native trusted style scope markers isolated per block root', async () => {
    const firstRoot = createBlockRoot();
    const secondRoot = createBlockRoot();
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => () => <div data-testid="native-block">Ready</div>
    });

    await adapter.mount({
      plan: createPlan({ blockId: 'native-block-first' }),
      root: firstRoot
    });
    await adapter.mount({
      plan: createPlan({ blockId: 'native-block-second' }),
      root: secondRoot
    });

    expect(firstRoot).toHaveAttribute(
      'data-flowbase-native-trusted-block-id',
      'native-block-first'
    );
    expect(secondRoot).toHaveAttribute(
      'data-flowbase-native-trusted-block-id',
      'native-block-second'
    );
  });

  test('restores only adapter-written style scope markers on dispose', async () => {
    const root = createBlockRoot();
    root.setAttribute('data-flowbase-native-trusted-block-root', 'preexisting');
    root.setAttribute(
      'data-flowbase-native-trusted-block-id',
      'preexisting-block'
    );
    root.setAttribute('data-host-owned-attribute', 'keep-me');
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => () => (
        <div data-testid="native-block">Mounted</div>
      )
    });

    const mounted = await adapter.mount({
      plan: createPlan({ blockId: 'native-block-disposed' }),
      root
    });

    root.setAttribute('data-host-owned-attribute', 'changed-by-host');
    expect(root).toHaveAttribute('data-flowbase-native-trusted-block-root', '');
    expect(root).toHaveAttribute(
      'data-flowbase-native-trusted-block-id',
      'native-block-disposed'
    );

    mounted?.dispose?.();

    expect(root).toHaveAttribute(
      'data-flowbase-native-trusted-block-root',
      'preexisting'
    );
    expect(root).toHaveAttribute(
      'data-flowbase-native-trusted-block-id',
      'preexisting-block'
    );
    expect(root).toHaveAttribute(
      'data-host-owned-attribute',
      'changed-by-host'
    );
  });

  test('resolves scoped AntD theme and locale independently for each block', async () => {
    const firstRoot = createBlockRoot();
    const secondRoot = createBlockRoot();
    const testingRoot = createTestingRoot();
    const firstTheme = { token: { colorPrimary: '#1677ff' } };
    const secondTheme = { token: { colorPrimary: '#52c41a' } };
    const firstLocale = {
      locale: 'en_US',
      Empty: { description: 'First empty' }
    };
    const secondLocale = {
      locale: 'zh_CN',
      Empty: { description: 'Second empty' }
    };
    const scopeResolver = vi.fn((context) => {
      if (context.plan.blockId === 'native-block-1') {
        return { theme: firstTheme, locale: firstLocale };
      }

      return { theme: secondTheme, locale: secondLocale };
    });
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveProviderScope: scopeResolver,
      resolveComponent: () => () => <div data-testid="native-block">Ready</div>
    });

    await adapter.mount({
      plan: createPlan({ blockId: 'native-block-1' }),
      root: firstRoot
    });
    await adapter.mount({
      plan: createPlan({ blockId: 'native-block-2' }),
      root: secondRoot
    });

    expect(scopeResolver).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        plan: expect.objectContaining({ blockId: 'native-block-1' }),
        root: firstRoot,
        shadowRoot: getShadowRoot(firstRoot),
        portalContainment: expect.objectContaining({
          root: getShadowRoot(firstRoot)
        })
      })
    );
    expect(scopeResolver).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        plan: expect.objectContaining({ blockId: 'native-block-2' }),
        root: secondRoot,
        shadowRoot: getShadowRoot(secondRoot),
        portalContainment: expect.objectContaining({
          root: getShadowRoot(secondRoot)
        })
      })
    );
    expect(antdProviderRecords.configProviderProps).toEqual([
      expect.objectContaining({
        theme: firstTheme,
        locale: firstLocale,
        getPopupContainer: expect.any(Function)
      }),
      expect.objectContaining({
        theme: secondTheme,
        locale: secondLocale,
        getPopupContainer: expect.any(Function)
      })
    ]);
    expect(
      (
        antdProviderRecords.configProviderProps[0]
          .getPopupContainer as () => ShadowRoot
      )()
    ).toBe(getShadowRoot(firstRoot));
    expect(
      (
        antdProviderRecords.configProviderProps[1]
          .getPopupContainer as () => ShadowRoot
      )()
    ).toBe(getShadowRoot(secondRoot));
  });

  test('lets providerWrapper wrap the default scoped provider with context access', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const providerWrapper = vi.fn((children: ReactNode, context) => (
      <section
        data-block-id={context.plan.blockId}
        data-testid="provider-wrapper"
      >
        {children}
      </section>
    ));
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      providerWrapper,
      resolveProviderScope: () => ({
        theme: { token: { colorPrimary: '#722ed1' } },
        locale: { locale: 'en_US' }
      }),
      resolveComponent: () => () => <div data-testid="native-block">Ready</div>
    });

    await adapter.mount({
      plan: createPlan({ blockId: 'native-block-wrapped' }),
      root
    });

    expect(
      await shadowQueries(root).findByTestId('provider-wrapper')
    ).toHaveAttribute('data-block-id', 'native-block-wrapped');
    expect(providerWrapper).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        plan: expect.objectContaining({ blockId: 'native-block-wrapped' }),
        root,
        shadowRoot: getShadowRoot(root),
        mountElement: expect.any(HTMLElement),
        portalContainment: expect.objectContaining({
          root: getShadowRoot(root)
        })
      })
    );
    expect(antdProviderRecords.configProviderProps).toEqual([
      expect.objectContaining({
        theme: { token: { colorPrimary: '#722ed1' } },
        locale: { locale: 'en_US' },
        getPopupContainer: expect.any(Function)
      })
    ]);
    expect(antdProviderRecords.appRenderCount).toBe(1);
  });

  test('passes plan props and portal containment to the resolved component', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const plan = createPlan({
      props: { title: 'Scoped block', nested: { ok: true } }
    });
    const received: unknown[] = [];
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => (props) => {
        received.push(props);

        return (
          <output data-testid="native-props">
            {props.props.title as string}:
            {String(props.portalContainment.root === getShadowRoot(root))}
          </output>
        );
      }
    });

    await adapter.mount({ plan, root });

    expect(
      await shadowQueries(root).findByTestId('native-props')
    ).toHaveTextContent('Scoped block:true');
    expect(received).toEqual([
      expect.objectContaining({
        plan,
        props: plan.props,
        portalContainment: expect.objectContaining({
          root: getShadowRoot(root),
          modal: expect.objectContaining({
            getContainer: expect.any(Function)
          }),
          select: expect.objectContaining({
            getPopupContainer: expect.any(Function)
          }),
          dropdown: expect.objectContaining({
            getPopupContainer: expect.any(Function)
          }),
          tooltip: expect.objectContaining({
            getPopupContainer: expect.any(Function)
          })
        })
      })
    ]);
  });

  test('passes a controlled default block context to the resolved component', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    let receivedContext: BlockContext | undefined;
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => (props) => {
        receivedContext = props.ctx;

        return (
          <output data-testid="native-context">
            {props.ctx.props.title as string}
          </output>
        );
      }
    });

    await adapter.mount({
      plan: createPlan({
        blockId: 'native-block-default-context',
        props: { title: 'Context title' }
      }),
      root
    });

    expect(
      await shadowQueries(root).findByTestId('native-context')
    ).toHaveTextContent('Context title');
    expect(receivedContext).toEqual(
      expect.objectContaining({
        currentUser: null,
        workspace: { id: 'workspace' },
        application: { id: 'application' },
        page: expect.objectContaining({
          id: 'native-block-default-context',
          route: 'native-block-default-context'
        }),
        params: {},
        props: { title: 'Context title' },
        state: {},
        theme: { mode: 'light', tokens: {} },
        ui: {}
      })
    );
    await expect(receivedContext?.api.get('/api/console/test')).rejects.toThrow(
      'Native trusted block ctx.api.get is unavailable until the host injects a controlled BlockContext.'
    );
    expect(() => receivedContext?.events.emit('record.opened')).toThrow(
      'Native trusted block ctx.events.emit is unavailable until the host injects a controlled BlockContext.'
    );
  });

  test('resolves a controlled block context per mount and injects it into component props', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const fakeContext = createFakeBlockContext({
      props: { title: 'Injected context' },
      api: {
        get: vi.fn(async () => ({
          title: 'Queried title'
        })) as BlockContext['api']['get'],
        post: vi.fn(),
        put: vi.fn(),
        patch: vi.fn(),
        delete: vi.fn(),
        head: vi.fn(),
        options: vi.fn(),
        stream: vi.fn()
      }
    });
    const resolveBlockContext = vi.fn(() => fakeContext);
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveBlockContext,
      resolveComponent: () => (props) => {
        void props.ctx.api.get('/api/console/test', {
          body: { blockId: props.plan.blockId }
        });

        return (
          <output data-testid="native-injected-context">
            {props.ctx.props.title as string}
          </output>
        );
      }
    });

    await adapter.mount({
      plan: createPlan({ blockId: 'native-block-ctx' }),
      root
    });

    expect(
      await shadowQueries(root).findByTestId('native-injected-context')
    ).toHaveTextContent('Injected context');
    expect(resolveBlockContext).toHaveBeenCalledWith(
      expect.objectContaining({
        plan: expect.objectContaining({ blockId: 'native-block-ctx' }),
        root,
        shadowRoot: getShadowRoot(root),
        mountElement: expect.any(HTMLElement),
        portalContainment: expect.objectContaining({
          root: getShadowRoot(root)
        })
      })
    );
    expect(fakeContext.api.get).toHaveBeenCalledWith('/api/console/test', {
      body: { blockId: 'native-block-ctx' }
    });
  });

  test('catches native render crashes and reports a runtime error with the current block id', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const onRuntimeError = vi.fn();
    const consoleErrorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      onRuntimeError,
      resolveComponent: () => () => {
        throw new Error('native render exploded');
      }
    });

    try {
      await adapter.mount({
        plan: createPlan({ blockId: 'native-block-crash' }),
        root
      });

      await waitFor(() => {
        expect(onRuntimeError).toHaveBeenCalledWith(
          expect.objectContaining({
            code: 'runtime_error',
            path: 'runtime.render',
            message: 'native render exploded'
          }),
          expect.objectContaining({
            blockId: 'native-block-crash',
            root,
            plan: expect.objectContaining({ blockId: 'native-block-crash' })
          })
        );
      });
    } finally {
      consoleErrorSpy.mockRestore();
    }
  });

  test('keeps one crashing native block scoped away from another mounted native block', async () => {
    const crashingRoot = createBlockRoot();
    const stableRoot = createBlockRoot();
    const testingRoot = createTestingRoot();
    const onRuntimeError = vi.fn();
    const consoleErrorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      onRuntimeError,
      resolveComponent: (plan) => {
        if (plan.blockId === 'native-block-crash') {
          return () => {
            throw new Error('first block failed');
          };
        }

        return () => <div data-testid="stable-native-block">Still mounted</div>;
      }
    });

    try {
      await adapter.mount({
        plan: createPlan({ blockId: 'native-block-crash' }),
        root: crashingRoot
      });
      await adapter.mount({
        plan: createPlan({ blockId: 'native-block-stable' }),
        root: stableRoot
      });

      expect(
        await shadowQueries(stableRoot).findByTestId('stable-native-block')
      ).toHaveTextContent('Still mounted');
      await waitFor(() => {
        expect(onRuntimeError).toHaveBeenCalledWith(
          expect.objectContaining({ code: 'runtime_error' }),
          expect.objectContaining({ blockId: 'native-block-crash' })
        );
      });
    } finally {
      consoleErrorSpy.mockRestore();
    }
  });

  test('D1-AC-003 keeps authored CSS, selectors, variables, and keyframes in each ShadowRoot', async () => {
    const firstRoot = createBlockRoot();
    const secondRoot = createBlockRoot();
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: (plan) => () => (
        <>
          <style>{`:root { --tone: ${plan.blockId === 'first' ? 'red' : 'blue'}; }
            @keyframes pulse { from { opacity: 0.5; } to { opacity: 1; } }
            .same { color: var(--tone); animation: pulse 1s; }`}</style>
          <div className="same" data-testid={`${plan.blockId}-same`} />
        </>
      )
    });

    await adapter.mount({
      plan: createPlan({ blockId: 'first' }),
      root: firstRoot
    });
    await adapter.mount({
      plan: createPlan({ blockId: 'second' }),
      root: secondRoot
    });

    const firstShadow = getShadowRoot(firstRoot);
    const secondShadow = getShadowRoot(secondRoot);
    expect(firstShadow).not.toBe(secondShadow);
    expect(firstShadow.querySelector('style')?.textContent).toContain(
      '--tone: red'
    );
    expect(secondShadow.querySelector('style')?.textContent).toContain(
      '--tone: blue'
    );
    expect(firstShadow.querySelector('[data-testid="second-same"]')).toBeNull();
    expect(secondShadow.querySelector('[data-testid="first-same"]')).toBeNull();
    expect(document.head.textContent).not.toContain('@keyframes pulse');
    expect(
      antdProviderRecords.styleProviderProps.map(({ container }) => container)
    ).toEqual([firstShadow, secondShadow]);
  });

  test('renders no raw crash details by default', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const consoleErrorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => () => {
        throw new Error('raw secret stack debug JSON prompt text');
      }
    });

    try {
      await adapter.mount({ plan: createPlan(), root });

      expect(getShadowRoot(root).textContent).not.toContain(
        'raw secret stack debug JSON prompt text'
      );
      expect(getShadowRoot(root).textContent).not.toContain('Error:');
      expect(getShadowRoot(root).textContent).not.toContain('runtime.render');
      expect(getShadowRoot(root).textContent).not.toContain('{');
    } finally {
      consoleErrorSpy.mockRestore();
    }
  });

  test('unmounts exactly once when dispose is called repeatedly', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => () => (
        <div data-testid="native-block">Mounted</div>
      )
    });

    const mounted = await adapter.mount({ plan: createPlan(), root });
    mounted?.dispose?.();
    mounted?.dispose?.();

    expect(testingRoot.unmountSpy).toHaveBeenCalledTimes(1);
    expect(getShadowRoot(root).childNodes).toHaveLength(0);
  });

  test('D1-AC-004 remounts the same block root after isolated idempotent cleanup', async () => {
    const root = createBlockRoot();
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: (plan) => () => (
        <div data-testid="native-retry">{plan.blockId}</div>
      )
    });

    const first = await adapter.mount({
      plan: createPlan({ blockId: 'before-retry' }),
      root
    });
    const shadowRoot = getShadowRoot(root);
    first?.dispose?.();
    first?.dispose?.();
    expect(shadowRoot.childNodes).toHaveLength(0);

    const second = await adapter.mount({
      plan: createPlan({ blockId: 'after-retry' }),
      root
    });
    expect(getShadowRoot(root)).toBe(shadowRoot);
    expect(
      await shadowQueries(root).findByTestId('native-retry')
    ).toHaveTextContent('after-retry');

    second?.dispose?.();
    expect(testingRoot.unmountSpy).toHaveBeenCalledTimes(2);
    expect(shadowRoot.childNodes).toHaveLength(0);
  });

  test('rejects invalid roots and resolver failures', async () => {
    const testingRoot = createTestingRoot();
    const adapter = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => () => null
    });

    await expect(
      adapter.mount({ plan: createPlan(), root: { nodeType: 1 } })
    ).rejects.toThrow(
      'Native trusted block React adapter root must be a DOM Element.'
    );
    expect(testingRoot.roots).toEqual([]);

    const resolverFailure = createFrontstageNativeTrustedBlockReactAdapter({
      createRoot: testingRoot.createRoot,
      resolveComponent: () => {
        throw new Error('resolver unavailable');
      }
    });

    await expect(
      resolverFailure.mount({ plan: createPlan(), root: createBlockRoot() })
    ).rejects.toThrow('resolver unavailable');
  });

  test('is not statically imported by existing frontstage pages, components, catalog, or route code', () => {
    const frontstageDir = join(process.cwd(), 'src/features/frontstage');
    const matches = collectSourceFiles([
      join(frontstageDir, 'pages'),
      join(frontstageDir, 'components'),
      join(frontstageDir, 'api'),
      join(frontstageDir, 'hooks'),
      join(frontstageDir, 'lib'),
      join(process.cwd(), 'src/routes'),
      join(process.cwd(), 'src/app')
    ]).filter(
      (filePath) =>
        filePath !== __filename &&
        !filePath.endsWith('native-trusted-block-runtime-factory.ts') &&
        readFileSync(filePath, 'utf8').includes(
          'native-trusted-block-react-adapter'
        )
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

      if (SOURCE_FILE_EXTENSIONS.has(extname(entry.name))) {
        files.push(entryPath);
      }
    }
  }

  return files;
}

const SOURCE_FILE_EXTENSIONS = new Set(['.ts', '.tsx']);
