import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { createStyles } from 'antd-style';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { act, useEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { BlockContext, BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';

import {
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent,
  type FrontstageNativeTrustedBlockReactComponentProps
} from '../../lib/native-trusted-block-react-adapter';
import {
  TrustedFrontendContributionHandle,
  type PreparedTrustedFrontendContribution
} from '../../lib/native-trusted-block-contribution-lifecycle';

const providerRecords = vi.hoisted(() => ({
  configs: [] as Array<Record<string, unknown>>,
  styles: [] as Array<Record<string, unknown>>
}));

afterEach(() => vi.unstubAllGlobals());

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
      providerRecords.styles.push(props);
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
      providerRecords.configs.push(props);
      return React.createElement(React.Fragment, null, children);
    },
    App({ children }: { children?: ReactNode }) {
      return React.createElement(React.Fragment, null, children);
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
    props: { title: 'Initial' },
    requiredPermissions: ['ui_block.javascript.native'],
    ...overrides
  };
}

function createContext(
  overrides: Partial<BlockContextSeed> = {}
): BlockContextSeed {
  return {
    currentUser: null,
    workspace: { id: 'workspace-1' },
    application: null,
    page: { id: 'page-1', route: '/page-1' },
    inputs: {},
    outputs: { publish: vi.fn() },
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
    events: { emit: vi.fn() },
    navigation: { openBlock: vi.fn() },
    theme: { mode: 'light', tokens: {} },
    ui: {},
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

describe('frontstage native trusted block declarative portal host', () => {
  beforeEach(() => {
    providerRecords.configs = [];
    providerRecords.styles = [];
  });

  test('D3R-AC-001 renders two ShadowRoot portals from one owner tree without a per-block React root', async () => {
    const firstRoot = createBlockRoot();
    const secondRoot = createBlockRoot();
    const Block: FrontstageNativeTrustedBlockReactComponent = ({ plan }) => (
      <output data-testid={plan.blockId}>{plan.blockId}</output>
    );

    render(
      <>
        <FrontstageNativeTrustedBlockPortalHost
          root={firstRoot}
          renderEpoch="first:1"
          plan={createPlan({ blockId: 'first' })}
          component={Block}
          ctx={createContext()}
        />
        <FrontstageNativeTrustedBlockPortalHost
          root={secondRoot}
          renderEpoch="second:1"
          plan={createPlan({ blockId: 'second' })}
          component={Block}
          ctx={createContext()}
        />
      </>
    );

    expect(
      await shadowQueries(firstRoot).findByTestId('first')
    ).toHaveTextContent('first');
    expect(
      await shadowQueries(secondRoot).findByTestId('second')
    ).toHaveTextContent('second');
    expect(firstRoot.shadowRoot).not.toBe(secondRoot.shadowRoot);
    const shadowStylePrefixes = providerRecords.configs.map(
      (config) => config.prefixCls
    );
    expect(shadowStylePrefixes).toHaveLength(2);
    expect(
      shadowStylePrefixes.every((prefix) => typeof prefix === 'string')
    ).toBe(true);
    expect(new Set(shadowStylePrefixes).size).toBe(2);

    const source = readFileSync(
      join(
        process.cwd(),
        'src/features/frontstage/lib/native-trusted-block-react-adapter.tsx'
      ),
      'utf8'
    );
    expect(source).toContain("from 'react-dom'");
    expect(source).not.toContain("from 'react-dom/client'");
    expect(source).not.toMatch(/\bcreateRoot\b/u);
    expect(source).not.toContain('.render(');
    expect(source).not.toContain('.unmount(');
  });

  test('D3R-AC-001 preserves component identity across plan, context, and theme updates', async () => {
    const root = createBlockRoot();
    const mounted = vi.fn();
    const unmounted = vi.fn();
    const StatefulBlock: FrontstageNativeTrustedBlockReactComponent = ({
      props,
      ctx
    }) => {
      const identity = useRef(Symbol('component'));
      const [count, setCount] = useState(0);
      useEffect(() => {
        mounted();
        return unmounted;
      }, []);
      return (
        <button
          data-identity={String(identity.current)}
          onClick={() => setCount(1)}
        >
          {count}:{String(props.title)}:{String(ctx.props.contextTitle)}
        </button>
      );
    };
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="stable-epoch"
        plan={createPlan()}
        component={StatefulBlock}
        ctx={createContext({ props: { contextTitle: 'Context 1' } })}
        moduleSources={['antd-style']}
        providerScope={{ theme: { token: { colorPrimary: '#111111' } } }}
      />
    );
    const button = await shadowQueries(root).findByRole('button');
    fireEvent.click(button);

    view.rerender(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="stable-epoch"
        plan={createPlan({ props: { title: 'Updated' } })}
        component={StatefulBlock}
        ctx={createContext({ props: { contextTitle: 'Context 2' } })}
        moduleSources={['antd-style']}
        providerScope={{ theme: { token: { colorPrimary: '#222222' } } }}
      />
    );

    await waitFor(() =>
      expect(button).toHaveTextContent('1:Updated:Context 2')
    );
    expect(mounted).toHaveBeenCalledTimes(1);
    expect(unmounted).not.toHaveBeenCalled();
  });

  test('I1927-AC-001 injects the responsive motion budget into the native surface theme', async () => {
    const root = createBlockRoot();

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="motion:responsive"
        plan={createPlan()}
        component={() => <output>responsive motion</output>}
        ctx={createContext()}
      />
    );

    await shadowQueries(root).findByText('responsive motion');
    expect(providerRecords.configs.at(-1)?.theme).toEqual({
      token: expect.objectContaining({
        motion: true,
        motionDurationFast: '0.03s',
        motionDurationMid: '0.05s',
        motionDurationSlow: '0.08s'
      })
    });
  });

  test('I1927-AC-002 preserves authored theme tokens over the responsive defaults', async () => {
    const root = createBlockRoot();

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="motion:authored"
        plan={createPlan()}
        component={() => <output>authored motion</output>}
        ctx={createContext()}
        providerScope={{
          theme: {
            token: {
              colorPrimary: '#123456',
              motionDurationMid: '0.24s'
            }
          }
        }}
      />
    );

    await shadowQueries(root).findByText('authored motion');
    expect(providerRecords.configs.at(-1)?.theme).toEqual({
      token: expect.objectContaining({
        colorPrimary: '#123456',
        motionDurationFast: '0.03s',
        motionDurationMid: '0.24s',
        motionDurationSlow: '0.08s'
      })
    });
  });

  test('I1927-AC-003 follows reduced-motion changes without remounting the block', async () => {
    const root = createBlockRoot();
    const mounted = vi.fn();
    let reduced = true;
    let changeListener: ((event: MediaQueryListEvent) => void) | undefined;
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        get matches() {
          return reduced;
        },
        media: '(prefers-reduced-motion: reduce)',
        onchange: null,
        addEventListener: vi.fn(
          (_type: string, listener: (event: MediaQueryListEvent) => void) => {
            changeListener = listener;
          }
        ),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
        dispatchEvent: vi.fn()
      }))
    );
    const Block = () => {
      useEffect(() => mounted(), []);
      return <output>reduced motion</output>;
    };

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="motion:reduced"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );

    await shadowQueries(root).findByText('reduced motion');
    expect(
      (
        providerRecords.configs.at(-1)?.theme as {
          token?: Record<string, unknown>;
        }
      ).token
    ).toEqual(
      expect.objectContaining({
        motion: false,
        motionDurationFast: '0s',
        motionDurationMid: '0s',
        motionDurationSlow: '0s'
      })
    );

    reduced = false;
    act(() => changeListener?.({ matches: false } as MediaQueryListEvent));

    await waitFor(() =>
      expect(
        (
          providerRecords.configs.at(-1)?.theme as {
            token?: Record<string, unknown>;
          }
        ).token
      ).toEqual(
        expect.objectContaining({
          motion: true,
          motionDurationFast: '0.03s',
          motionDurationMid: '0.05s',
          motionDurationSlow: '0.08s'
        })
      )
    );
    expect(mounted).toHaveBeenCalledTimes(1);
  });

  test('D5-P2 mounts once, updates without remounting, and disposes the typed contribution instance once', async () => {
    const root = createBlockRoot();
    const contribution = preparedContribution();
    const mount = vi.spyOn(
      TrustedFrontendContributionHandle.prototype,
      'mount'
    );
    const update = vi.spyOn(
      TrustedFrontendContributionHandle.prototype,
      'update'
    );
    const dispose = vi.spyOn(
      TrustedFrontendContributionHandle.prototype,
      'dispose'
    );
    const Block = ({ props }: { props: Record<string, unknown> }) => (
      <output>{String(props.title)}</output>
    );
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="typed:1"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
        contribution={contribution}
      />
    );
    await shadowQueries(root).findByText('Initial');

    view.rerender(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="typed:1"
        plan={createPlan({ props: { title: 'Updated' } })}
        component={Block}
        ctx={createContext()}
        contribution={contribution}
      />
    );
    await shadowQueries(root).findByText('Updated');
    expect(mount).toHaveBeenCalledOnce();
    expect(update).toHaveBeenCalled();

    view.unmount();
    expect(dispose).toHaveBeenCalledOnce();
    mount.mockRestore();
    update.mockRestore();
    dispose.mockRestore();
  });

  test('D3R-AC-007 scopes providers, authored CSS, and popup containment to the block ShadowRoot', async () => {
    const root = createBlockRoot();
    const receivedContainment: unknown[] = [];
    const Block: FrontstageNativeTrustedBlockReactComponent = ({
      portalContainment
    }) => {
      receivedContainment.push(portalContainment);
      return (
        <>
          <style>{'.native-same { color: var(--native-tone); }'}</style>
          <div className="native-same">Contained</div>
        </>
      );
    };

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="contained:1"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );

    await shadowQueries(root).findByText('Contained');
    const shadowRoot = root.shadowRoot;
    expect(providerRecords.styles).toEqual([
      expect.objectContaining({ container: shadowRoot })
    ]);
    const config = providerRecords.configs[0];
    expect((config.getPopupContainer as () => ShadowRoot)()).toBe(shadowRoot);
    expect((config.getTargetContainer as () => Window)()).toBe(window);
    expect(receivedContainment).toEqual([
      expect.objectContaining({
        root: shadowRoot,
        modal: expect.objectContaining({ getContainer: expect.any(Function) }),
        tooltip: expect.objectContaining({
          getPopupContainer: expect.any(Function)
        })
      })
    ]);
    expect(document.head).not.toHaveTextContent(/\.native-same/);
  });

  test('I1907-AC-001/002 injects antd-style rules into the importing block ShadowRoot', async () => {
    const root = createBlockRoot();
    const useStyles = createStyles({
      shell: {
        border: '3px solid rgb(22, 119, 255)',
        padding: 17
      }
    });
    const Block = () => {
      const { styles } = useStyles();
      return <output className={styles.shell}>antd-style scoped</output>;
    };

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="antd-style:1"
        plan={createPlan({
          source:
            "import { createStyles } from 'antd-style'; export default function Block() { return null; }"
        })}
        component={Block}
        ctx={createContext()}
        moduleSources={['antd-style']}
      />
    );

    const output = await shadowQueries(root).findByText('antd-style scoped');
    expect(output.className).toMatch(/css-/u);
    expect(
      root.shadowRoot?.querySelector('style[data-emotion]')
    ).not.toBeNull();
    expect(document.head.querySelector(`style[data-emotion]`)).toBeNull();
  });

  test('I1907-AC-002/005 isolates antd-style caches for two surfaces and cleans them on dispose', async () => {
    const firstRoot = createBlockRoot();
    const secondRoot = createBlockRoot();
    const useStyles = createStyles({
      surface: { backgroundColor: 'rgb(245, 245, 245)' }
    });
    const Block = ({
      plan
    }: FrontstageNativeTrustedBlockReactComponentProps) => {
      const { styles } = useStyles();
      return <output className={styles.surface}>{plan.entry}</output>;
    };
    const view = render(
      <>
        <FrontstageNativeTrustedBlockPortalHost
          root={firstRoot}
          renderEpoch="antd-style:first"
          plan={createPlan({ entry: 'First' })}
          component={Block}
          ctx={createContext()}
          moduleSources={['antd-style']}
        />
        <FrontstageNativeTrustedBlockPortalHost
          root={secondRoot}
          renderEpoch="antd-style:second"
          plan={createPlan({ entry: 'Second' })}
          component={Block}
          ctx={createContext()}
          moduleSources={['antd-style']}
        />
      </>
    );

    await shadowQueries(firstRoot).findByText('First');
    await shadowQueries(secondRoot).findByText('Second');
    const firstStyle = firstRoot.shadowRoot?.querySelector(
      'style[data-emotion]'
    );
    const secondStyle = secondRoot.shadowRoot?.querySelector(
      'style[data-emotion]'
    );
    expect(firstStyle?.getAttribute('data-emotion')).not.toBe(
      secondStyle?.getAttribute('data-emotion')
    );
    expect(document.head.querySelector('style[data-emotion]')).toBeNull();

    view.unmount();
    expect(firstRoot.shadowRoot?.childNodes).toHaveLength(0);
    expect(secondRoot.shadowRoot?.childNodes).toHaveLength(0);
  });

  test('AC-003/006 injects ctx.assets into the current ShadowRoot and disposes its resources on unmount', async () => {
    const root = createBlockRoot();
    const receivedContexts: BlockContext[] = [];
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        status: 200,
        text: async () =>
          '<svg><symbol id="icon-scoped" viewBox="0 0 16 16"><path d="M0 0h16v16H0z" /></symbol></svg>'
      }))
    );
    const Block: FrontstageNativeTrustedBlockReactComponent = ({ ctx }) => {
      receivedContexts.push(ctx);
      useEffect(() => {
        void ctx.assets.loadSvgSprite('https://cdn.example.test/icons.svg');
      }, [ctx]);
      return <output>Scoped assets ready</output>;
    };

    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="assets:1"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );

    await shadowQueries(root).findByText('Scoped assets ready');
    await waitFor(() =>
      expect(root.shadowRoot?.querySelector('#icon-scoped')).not.toBeNull()
    );
    expect(receivedContexts.at(-1)?.root).toBe(root.shadowRoot);
    expect(receivedContexts.at(-1)?.assets).toEqual(
      expect.objectContaining({
        importModule: expect.any(Function),
        loadStyle: expect.any(Function),
        loadScript: expect.any(Function),
        loadSvgSprite: expect.any(Function)
      })
    );

    view.unmount();
    expect(root.shadowRoot?.querySelector('#icon-scoped')).toBeNull();
  });

  test('D3R-AC-007 contains a render error to the current portal', async () => {
    const crashingRoot = createBlockRoot();
    const stableRoot = createBlockRoot();
    const onRuntimeError = vi.fn();
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const Crash = () => {
      throw new Error('portal render exploded');
    };
    const Stable = () => <output>Still present</output>;

    try {
      render(
        <>
          <FrontstageNativeTrustedBlockPortalHost
            root={crashingRoot}
            renderEpoch="crash:1"
            plan={createPlan({ blockId: 'crash' })}
            component={Crash}
            ctx={createContext()}
            onRuntimeError={onRuntimeError}
          />
          <FrontstageNativeTrustedBlockPortalHost
            root={stableRoot}
            renderEpoch="stable:1"
            plan={createPlan({ blockId: 'stable' })}
            component={Stable}
            ctx={createContext()}
          />
        </>
      );

      expect(
        await shadowQueries(stableRoot).findByText('Still present')
      ).toBeInTheDocument();
      await waitFor(() =>
        expect(onRuntimeError).toHaveBeenCalledWith(
          expect.objectContaining({
            code: 'runtime_error',
            path: 'runtime.render',
            message: 'portal render exploded'
          }),
          expect.objectContaining({ blockId: 'crash', root: crashingRoot })
        )
      );
      expect(crashingRoot.shadowRoot).not.toHaveTextContent(
        'portal render exploded'
      );
    } finally {
      consoleError.mockRestore();
    }
  });

  test('attaches verified module styles to each ShadowRoot and removes them on unmount', async () => {
    const firstRoot = createBlockRoot();
    const secondRoot = createBlockRoot();
    const view = render(
      <>
        <FrontstageNativeTrustedBlockPortalHost
          root={firstRoot}
          renderEpoch="styles:first"
          plan={createPlan({ blockId: 'first' })}
          component={() => <output className="same">First</output>}
          ctx={createContext()}
          moduleAssets={[moduleStyle('a', '.same { color: red; }')]}
        />
        <FrontstageNativeTrustedBlockPortalHost
          root={secondRoot}
          renderEpoch="styles:second"
          plan={createPlan({ blockId: 'second' })}
          component={() => <output className="same">Second</output>}
          ctx={createContext()}
          moduleAssets={[moduleStyle('b', '.same { color: blue; }')]}
        />
      </>
    );

    await shadowQueries(firstRoot).findByText('First');
    await shadowQueries(secondRoot).findByText('Second');
    expect(firstRoot.shadowRoot?.textContent).toContain('color: red');
    expect(firstRoot.shadowRoot?.textContent).not.toContain('color: blue');
    expect(secondRoot.shadowRoot?.textContent).toContain('color: blue');
    expect(secondRoot.shadowRoot?.textContent).not.toContain('color: red');
    expect(document.head).not.toHaveTextContent(/color: red/);
    expect(document.head).not.toHaveTextContent(/color: blue/);

    view.unmount();
    expect(firstRoot.shadowRoot?.childNodes).toHaveLength(0);
    expect(secondRoot.shadowRoot?.childNodes).toHaveLength(0);
  });

  test('AC-003 shares one constructable stylesheet by asset digest across ShadowRoots', async () => {
    const replaceSync = vi.fn();
    class SharedStyleSheet {
      replaceSync(css: string) {
        replaceSync(css);
      }
    }
    vi.stubGlobal('CSSStyleSheet', SharedStyleSheet);
    const previousStyleSheetDescriptor = Object.getOwnPropertyDescriptor(
      window,
      'CSSStyleSheet'
    );
    Object.defineProperty(window, 'CSSStyleSheet', {
      configurable: true,
      value: SharedStyleSheet
    });
    const adoptedSheets = new WeakMap<ShadowRoot, CSSStyleSheet[]>();
    const previousDescriptor = Object.getOwnPropertyDescriptor(
      ShadowRoot.prototype,
      'adoptedStyleSheets'
    );
    Object.defineProperty(ShadowRoot.prototype, 'adoptedStyleSheets', {
      configurable: true,
      get(this: ShadowRoot) {
        return adoptedSheets.get(this) ?? [];
      },
      set(this: ShadowRoot, sheets: CSSStyleSheet[]) {
        adoptedSheets.set(this, sheets);
      }
    });
    const firstRoot = createBlockRoot();
    const secondRoot = createBlockRoot();
    const sharedStyle = moduleStyle('c', '.shared { display: grid; }');
    try {
      const view = render(
        <>
          <FrontstageNativeTrustedBlockPortalHost
            root={firstRoot}
            renderEpoch="shared:first"
            plan={createPlan({ blockId: 'shared-first' })}
            component={() => <output>First shared</output>}
            ctx={createContext()}
            moduleAssets={[sharedStyle]}
          />
          <FrontstageNativeTrustedBlockPortalHost
            root={secondRoot}
            renderEpoch="shared:second"
            plan={createPlan({ blockId: 'shared-second' })}
            component={() => <output>Second shared</output>}
            ctx={createContext()}
            moduleAssets={[sharedStyle]}
          />
        </>
      );

      await shadowQueries(firstRoot).findByText('First shared');
      await shadowQueries(secondRoot).findByText('Second shared');
      expect(replaceSync).toHaveBeenCalledTimes(1);
      expect(firstRoot.shadowRoot?.adoptedStyleSheets).toHaveLength(1);
      expect(secondRoot.shadowRoot?.adoptedStyleSheets[0]).toBe(
        firstRoot.shadowRoot?.adoptedStyleSheets[0]
      );

      view.unmount();
      expect(firstRoot.shadowRoot?.adoptedStyleSheets).toHaveLength(0);
      expect(secondRoot.shadowRoot?.adoptedStyleSheets).toHaveLength(0);
    } finally {
      if (previousStyleSheetDescriptor) {
        Object.defineProperty(
          window,
          'CSSStyleSheet',
          previousStyleSheetDescriptor
        );
      } else {
        Reflect.deleteProperty(window, 'CSSStyleSheet');
      }
      if (previousDescriptor) {
        Object.defineProperty(
          ShadowRoot.prototype,
          'adoptedStyleSheets',
          previousDescriptor
        );
      } else {
        Reflect.deleteProperty(ShadowRoot.prototype, 'adoptedStyleSheets');
      }
    }
  });

  test('D3R-AC-008 surface unmount cleans portal DOM and host-owned scope', async () => {
    const root = createBlockRoot();
    root.setAttribute('data-flowbase-native-trusted-block-id', 'before-host');
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="cleanup:1"
        plan={createPlan({ blockId: 'cleanup' })}
        component={() => <output>Mounted</output>}
        ctx={createContext()}
      />
    );

    await shadowQueries(root).findByText('Mounted');
    view.unmount();

    expect(root.shadowRoot?.childNodes).toHaveLength(0);
    expect(root).not.toHaveAttribute('data-flowbase-native-trusted-block-root');
    expect(root).toHaveAttribute(
      'data-flowbase-native-trusted-block-id',
      'before-host'
    );
  });

  test('D3R-AC-008 lets React remove direct ShadowRoot portal children before disposing the surface', async () => {
    const root = createBlockRoot();
    const DirectShadowPortal: FrontstageNativeTrustedBlockReactComponent = ({
      portalContainment
    }) => {
      if (!(portalContainment.root instanceof ShadowRoot)) {
        throw new Error('Expected a ShadowRoot portal containment handle.');
      }
      return createPortal(
        <output data-testid="direct-shadow-child">Direct child</output>,
        portalContainment.root
      );
    };
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="cleanup-order:1"
        plan={createPlan({ blockId: 'cleanup-order' })}
        component={DirectShadowPortal}
        ctx={createContext()}
      />
    );

    await shadowQueries(root).findByTestId('direct-shadow-child');

    expect(() => view.unmount()).not.toThrow();
    expect(root.shadowRoot?.childNodes).toHaveLength(0);
    expect(root).not.toHaveAttribute('data-flowbase-native-trusted-block-root');
  });

  test('an explicit render epoch remounts exactly once for retry or identity change', async () => {
    const root = createBlockRoot();
    const mounted = vi.fn();
    const unmounted = vi.fn();
    const Block = () => {
      useEffect(() => {
        mounted();
        return unmounted;
      }, []);
      return <output>Epoch</output>;
    };
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="epoch:1"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );
    await shadowQueries(root).findByText('Epoch');

    view.rerender(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="epoch:2"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );

    await waitFor(() => {
      expect(mounted).toHaveBeenCalledTimes(2);
      expect(unmounted).toHaveBeenCalledTimes(1);
    });
  });
});

function moduleStyle(digestCharacter: string, css: string) {
  return {
    module_source: '@1flowbase/native-components',
    role: 'shadow_style' as const,
    media_type: 'text/css; charset=utf-8',
    sha256: digestCharacter.repeat(64),
    url: `/assets/${digestCharacter}`,
    bytes: new TextEncoder().encode(css).buffer as ArrayBuffer
  };
}

function preparedContribution(): PreparedTrustedFrontendContribution {
  const contribution: PreparedTrustedFrontendContribution = {
    state: 'prepared',
    contributionId: 'frontend-block.installation-1.hero',
    blockId: 'installation-1:hero',
    blockVersion: '1.0.0',
    assetIntegrity: ['verified_sha256'],
    grantedPermissions: ['frontend-block.ui-mount.trusted-host'],
    graphFingerprint: 'graph-fingerprint',
    runtimeKind: 'trusted_native',
    executionKind: 'ui_mount',
    isolationRequirement: 'trusted_host_realm',
    lifecycleKind: 'workspace_assignment',
    createHandle: () => new TrustedFrontendContributionHandle(contribution)
  };
  return contribution;
}
