import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import {
  LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC,
  compileNativeReactComponent,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime';

import { appI18n } from '../../../shared/i18n/app-i18n';
import { JsBlockTrialPanel } from '../components/JsBlockTrialPanel';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import type { FrontstageBlockInstance } from '../lib/page-document';
import type { NativeReactBrowserCompileResult } from '../../../shared/code-block/native-react-compiler-browser';
import { createFrontstageUnavailableBlockContext } from '../lib/native-trusted-block-react-adapter';

const block = {
  id: 'block-1',
  rendererVersion: 'v1',
  sourceId: 'block-1',
  codeRef: 'code-1',
  sourceCodeRef: 'code-1',
  catalog: { providerCode: 'official', installationId: 'installation-1' },
  contribution: {
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    code: 'tsx'
  },
  props: {},
  ports: { inputs: [], outputs: [] },
  presentation: { heightMode: 'auto', height: null },
  layout: { order: 0 },
  order: 0,
  runtime: { kind: 'native_react', entry: 'index.js', hint: 'native_react' }
} satisfies FrontstageBlockInstance;

const catalog = {
  id: 'official:tsx',
  runtimeKind: 'native_react',
  installationId: 'installation-1',
  providerCode: 'official',
  pluginId: 'official.blocks',
  pluginVersion: '1.0.0',
  contributionCode: 'tsx',
  title: 'TSX',
  entry: 'index.js',
  permissions: { network: 'none', storage: 'none', secrets: 'none' },
  contextContract: { primitives: [], inputSchema: {} },
  uiCapabilities: [],
  raw: {}
} as unknown as NormalizedFrontstageBlockCatalogEntry;

function source(label: string): string {
  return `export default function Block() {
    return <div data-testid="native-output">${label}</div>;
  }`;
}

function createCompiler() {
  return vi.fn(async ({ source: currentSource }: { source: string }) => {
    const result = compileNativeReactComponent(currentSource);
    return result as NativeReactBrowserCompileResult;
  });
}

function renderPanel({
  code,
  revision,
  nativeCompiler = createCompiler(),
  nativeDependencyLock,
  nativeDependencyLockError,
  currentBlock = block
}: {
  code: string;
  revision: string;
  nativeCompiler?: ReturnType<typeof createCompiler>;
  nativeDependencyLock?: NativeReactCatalogDependencyLock;
  nativeDependencyLockError?: string | null;
  currentBlock?: FrontstageBlockInstance;
}) {
  return render(
    <JsBlockTrialPanel
      block={currentBlock}
      catalogEntry={catalog}
      code={code}
      revision={revision}
      nativeCompiler={nativeCompiler}
      nativeDependencyLock={nativeDependencyLock}
      nativeDependencyLockError={nativeDependencyLockError}
    />
  );
}

function trialShadowRoot(container: HTMLElement): ShadowRoot {
  const host = container.querySelector<HTMLElement>(
    '[data-testid="native-react-trial-root"]'
  );
  if (!host?.shadowRoot) throw new Error('Expected native trial ShadowRoot.');
  return host.shadowRoot;
}

function trialQueries(container: HTMLElement) {
  return within(trialShadowRoot(container) as unknown as HTMLElement);
}

describe('JsBlockTrialPanel Native React run revision', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
  });

  test('D1-AC-001/005 freezes code per revision and remounts only for a new run revision', async () => {
    const compiler = createCompiler();
    const view = renderPanel({
      code: source('first'),
      revision: 'run:1',
      nativeCompiler: compiler
    });

    await waitFor(() =>
      expect(
        view.container.querySelector<HTMLElement>(
          '[data-testid="native-react-trial-root"]'
        )?.shadowRoot
      ).not.toBeNull()
    );
    await trialQueries(view.container).findByTestId('native-output');
    expect(
      trialQueries(view.container).getByTestId('native-output')
    ).toHaveTextContent('first');
    expect(compiler).toHaveBeenCalledTimes(1);

    view.rerender(
      <JsBlockTrialPanel
        block={block}
        catalogEntry={catalog}
        code={source('edited without run')}
        revision="run:1"
        nativeCompiler={compiler}
      />
    );
    await act(async () => Promise.resolve());
    expect(compiler).toHaveBeenCalledTimes(1);
    expect(
      trialQueries(view.container).getByTestId('native-output')
    ).toHaveTextContent('first');

    view.rerender(
      <JsBlockTrialPanel
        block={block}
        catalogEntry={catalog}
        code={source('second')}
        revision="run:2"
        nativeCompiler={compiler}
      />
    );
    await waitFor(() => expect(compiler).toHaveBeenCalledTimes(2));
    expect(
      await trialQueries(view.container).findByTestId('native-output')
    ).toHaveTextContent('second');
  });

  test('D2-P2F sends the catalog dependency lock through the production compiler input', async () => {
    const compiler = createCompiler();
    const dependencyLock: NativeReactCatalogDependencyLock = [
      {
        module_source: '@1flowbase/native-components',
        module_version: '1.0.0',
        browser_asset: {
          sha256:
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
          url: '/api/console/frontstage/workspace-1/component-module-assets/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        },
        exports: ['Surface']
      }
    ];
    renderPanel({
      code: "import { Surface } from '@1flowbase/native-components'; export default () => <Surface />;",
      revision: 'run:catalog-lock',
      nativeCompiler: compiler,
      nativeDependencyLock: dependencyLock
    });

    await waitFor(() => expect(compiler).toHaveBeenCalledTimes(1));
    expect(compiler).toHaveBeenCalledWith(
      expect.objectContaining({ dependencyLock })
    );
  });

  test('D2-P2F fails visibly before compilation when catalog metadata is incomplete', async () => {
    const compiler = createCompiler();
    renderPanel({
      code: "import { Surface } from '@1flowbase/native-components'; export default () => <Surface />;",
      revision: 'run:invalid-catalog-lock',
      nativeCompiler: compiler,
      nativeDependencyLockError:
        'Frontend block catalog dependency metadata is incomplete for this block.'
    });

    expect(await screen.findByText('运行失败')).toBeInTheDocument();
    expect(
      screen.getByText(
        'Frontend block catalog dependency metadata is incomplete for this block.'
      )
    ).toBeInTheDocument();
    expect(compiler).not.toHaveBeenCalled();
  });

  test('D1-AC-004 shows stable compile diagnostics and retries only the frozen source', async () => {
    const successful = compileNativeReactComponent(source('recovered'));
    if (!successful.ok)
      throw new Error('Expected successful compiler fixture.');
    const compiler = vi
      .fn()
      .mockResolvedValueOnce({
        ok: false,
        diagnostics: [
          {
            phase: 'compile',
            code: 'transform_failed',
            path: 'source.tsx',
            message: 'Malformed TSX',
            sourceLocation: { line: 2, column: 7 }
          }
        ]
      })
      .mockResolvedValueOnce({
        ok: true,
        artifact: successful.artifact,
        diagnostics: []
      });
    const view = renderPanel({
      code: source('frozen'),
      revision: 'run:compile-error',
      nativeCompiler: compiler
    });

    expect(await screen.findByText('运行失败')).toBeInTheDocument();
    expect(screen.getByText('Malformed TSX')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /重\s*试/u }));

    await waitFor(() => expect(compiler).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(
        view.container.querySelector<HTMLElement>(
          '[data-testid="native-react-trial-root"]'
        )?.shadowRoot
      ).not.toBeNull()
    );
    expect(
      await trialQueries(view.container).findByTestId('native-output')
    ).toHaveTextContent('recovered');
    expect(compiler.mock.calls[1]?.[0]).toMatchObject({
      source: source('frozen')
    });
  });

  test('D1-AC-004 confines render errors to the failing ShadowRoot', async () => {
    const compiler = createCompiler();
    const crashingBlock = { ...block, id: 'crashing-block' };
    const stableBlock = { ...block, id: 'stable-block' };
    const { container } = render(
      <>
        <JsBlockTrialPanel
          block={crashingBlock}
          catalogEntry={catalog}
          code="export default function Block() { throw new Error('render exploded'); }"
          revision="run:crash"
          nativeCompiler={compiler}
        />
        <JsBlockTrialPanel
          block={stableBlock}
          catalogEntry={catalog}
          code={source('stable')}
          revision="run:stable"
          nativeCompiler={compiler}
        />
      </>
    );

    const hosts = container.querySelectorAll<HTMLElement>(
      '[data-testid="native-react-trial-root"]'
    );
    await waitFor(() => expect(compiler).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(hosts[1]?.shadowRoot).not.toBeNull());
    expect(
      await within(hosts[1]!.shadowRoot as unknown as HTMLElement).findByTestId(
        'native-output'
      )
    ).toHaveTextContent('stable');
    expect(await screen.findByText(/render exploded/u)).toBeInTheDocument();
    expect(hosts[0]!.shadowRoot).not.toBe(hosts[1]!.shadowRoot);
  });

  test('D4-AC-001/003 binds the shared Native Host context and draft authorization without remounting for API calls', async () => {
    const apiPost = vi.fn().mockResolvedValue({ ok: true });
    const prepareDraftRun = vi.fn().mockResolvedValue(undefined);
    const revokeDraftRun = vi.fn();
    const code = `
      import { useState } from 'react';
      import { Button } from 'antd';
      export default function Block({ ctx }) {
        const [count, setCount] = useState(0);
        return <div>
          <span data-testid="studio-count">{count}</span>
          <Button onClick={() => setCount((value) => value + 1)}>Local</Button>
          <Button onClick={() => void ctx.api.post('/api/public/auth/sign-up')}>Register</Button>
        </div>;
      }
    `;
    const compiler = createCompiler();
    const view = render(
      <JsBlockTrialPanel
        block={block}
        catalogEntry={catalog}
        code={code}
        revision="run:auth-studio"
        nativeCompiler={compiler}
        onPrepareDraftRun={prepareDraftRun}
        onRevokeDraftRun={revokeDraftRun}
        createBlockContext={({ plan }) => {
          const context = createFrontstageUnavailableBlockContext(plan);
          return {
            ...context,
            api: { ...context.api, post: apiPost }
          };
        }}
      />
    );
    await waitFor(() => expect(prepareDraftRun).toHaveBeenCalledOnce());
    const queries = trialQueries(view.container);
    const local = await queries.findByRole('button', { name: 'Local' });
    fireEvent.click(queries.getByRole('button', { name: 'Register' }));
    fireEvent.click(local);

    await waitFor(() =>
      expect(apiPost).toHaveBeenCalledWith('/api/public/auth/sign-up')
    );
    expect(queries.getByTestId('studio-count')).toHaveTextContent('1');
    expect(compiler).toHaveBeenCalledOnce();
    view.unmount();
    expect(revokeDraftRun).toHaveBeenCalledWith(
      expect.stringMatching(/^draft:block-1:/u)
    );
  });

  test('D4-AC-006 rejects controlled legacy source before authorization or compilation', async () => {
    const legacySource = `async function main(ctx) { return { view: null, outputs: {} }; }
export default { main } satisfies BlockModule;`;
    const compiler = createCompiler();
    const prepareDraftRun = vi.fn();
    render(
      <JsBlockTrialPanel
        block={block}
        catalogEntry={catalog}
        code={legacySource}
        revision="run:legacy"
        nativeCompiler={compiler}
        onPrepareDraftRun={prepareDraftRun}
      />
    );

    expect(
      await screen.findByText(LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC.message)
    ).toBeInTheDocument();
    expect(compiler).not.toHaveBeenCalled();
    expect(prepareDraftRun).not.toHaveBeenCalled();
  });
});
