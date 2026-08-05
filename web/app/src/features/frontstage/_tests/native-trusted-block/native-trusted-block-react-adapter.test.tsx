import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type { BlockContext } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';

import {
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../../lib/native-trusted-block-react-adapter';

const providerRecords = vi.hoisted(() => ({
  configs: [] as Array<Record<string, unknown>>,
  styles: [] as Array<Record<string, unknown>>
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

function createContext(overrides: Partial<BlockContext> = {}): BlockContext {
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
        providerScope={{ theme: { token: { colorPrimary: '#222222' } } }}
      />
    );

    await waitFor(() =>
      expect(button).toHaveTextContent('1:Updated:Context 2')
    );
    expect(mounted).toHaveBeenCalledTimes(1);
    expect(unmounted).not.toHaveBeenCalled();
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
    expect((config.getTargetContainer as () => ShadowRoot)()).toBe(shadowRoot);
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
