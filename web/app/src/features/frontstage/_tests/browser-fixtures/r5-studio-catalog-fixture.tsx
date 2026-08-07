import type { BlockProtocolError } from '@1flowbase/page-protocol';
import {
  validateNativeTrustedBlockSource,
  type NativeBlockContextApiCallObservation,
  type NativeReactCatalogDependencyLock,
  type NativeReactCompileDiagnostic,
  type NativeReactRuntimeDiagnostic
} from '@1flowbase/page-runtime';
import { Button, ConfigProvider, Space, Typography } from 'antd';
import { useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { compileNativeReactComponentInBrowser } from '../../../../shared/code-block/native-react-compiler-browser';
import { JsxStudioPreviewConsole } from '../../components/jsx-studio/JsxStudioPreviewConsole';
import { JsxStudioRunPanel } from '../../components/jsx-studio/JsxStudioRunPanel';
import { createStudioRunConsoleStore } from '../../components/jsx-studio/studio-run-console';
import type {
  NormalizedFrontstageBlockCatalogEntry,
  NormalizedFrontstageBlockCodeModule
} from '../../lib/block-catalog';
import { createFrontstageJsxEditorProjection } from '../../lib/jsx-studio/editor-projection';
import type { FrontstageBlockInstance } from '../../lib/page-document';

const CATALOG_MODULE_SOURCES = [
  'react',
  'antd',
  '@1flowbase/block-sdk',
  '@1flowbase/native-components',
  '@ant-design/icons',
  '@1flowbase/charts',
  '@1flowbase/rich-text'
] as const;

const APPROVED_SOURCE = `${CATALOG_MODULE_SOURCES.map(
  (source) => `import '${source}';`
).join('\n')}

export default function FixtureBlock() {
  return <div>Approved Catalog imports</div>;
}`;
const DENIED_SOURCE = `import dayjs from 'dayjs';

export default function FixtureBlock() {
  return <div>{String(dayjs)}</div>;
}`;

const codeModules = CATALOG_MODULE_SOURCES.map(
  (source, index): NormalizedFrontstageBlockCodeModule => ({
    source,
    version: `fixture-${index + 1}.0.0`,
    binding: source === 'react' || source === 'antd' ? 'host' : 'fetched',
    assets:
      source === 'react' || source === 'antd'
        ? []
        : [
            {
              role: 'browser_module',
              media_type: 'text/javascript; charset=utf-8',
              sha256: (index + 1).toString(16).repeat(64)
            }
          ],
    exports: ['default'],
    type_declarations: `declare module '${source}' { const value: unknown; export default value; }`
  })
);
const dependencyLock: NativeReactCatalogDependencyLock = codeModules.map(
  (codeModule) => ({
    module_source: codeModule.source,
    module_version: codeModule.version,
    binding: codeModule.binding,
    assets: codeModule.assets.map((asset) => ({
      ...asset,
      url: `/fixture-assets/${encodeURIComponent(codeModule.source)}/${asset.sha256}`
    })),
    exports: [...codeModule.exports]
  })
);

const builtinEntry = createCatalogEntry({
  id: '1flowbase:frontstage.js-ui-block',
  title: 'Built-in block',
  installationId: 'builtin-installation',
  providerCode: '1flowbase',
  pluginId: 'builtin-frontstage',
  pluginVersion: '5.0.0',
  contributionCode: 'frontstage.js-ui-block',
  templateSource: 'export default function BuiltIn() { return <div />; }'
});
const runtimeDiagnostic: NativeReactRuntimeDiagnostic = {
  phase: 'runtime',
  code: 'runtime_error',
  path: 'fixture.runtime',
  message: 'Fixture runtime diagnostic'
};
const apiCalls: NativeBlockContextApiCallObservation[] = [
  {
    capability: 'api',
    requestId: 'fixture-run',
    instanceEpoch: 'fixture-epoch',
    callId: 'fixture-run:call-1',
    method: 'GET',
    path: '/api/console/example',
    status: 'pending',
    durationMs: 0
  },
  {
    capability: 'api',
    requestId: 'fixture-run',
    instanceEpoch: 'fixture-epoch',
    callId: 'fixture-run:call-1',
    method: 'GET',
    path: '/api/console/example',
    status: 'succeeded',
    durationMs: 12
  }
];
const consoleBlock = {
  id: 'r7-console-block',
  rendererVersion: 'v1',
  sourceId: 'r7-console-block',
  codeRef: 'r7-console-code',
  sourceCodeRef: 'r7-console-code',
  catalog: { providerCode: '1flowbase', installationId: 'builtin-installation' },
  contribution: {
    pluginId: 'builtin-frontstage',
    pluginVersion: '5.0.0',
    code: 'frontstage.js-ui-block'
  },
  props: {},
  ports: { inputs: [], outputs: [] },
  presentation: { heightMode: 'auto', height: null },
  layout: { order: 0 },
  order: 0,
  runtime: { kind: 'native_react', entry: 'index.js', hint: 'native_react' }
} satisfies FrontstageBlockInstance;
const consoleSource = `
import { useState } from 'react';
export default function ConsoleFixture() {
  const [count, setCount] = useState(0);
  console.log('browser render', count);
  return <button onClick={() => {
    console.warn('browser clicked', { count });
    setCount((value) => value + 1);
  }}>Emit runtime log</button>;
}`;

function R5StudioCatalogFixture() {
  const [source, setSource] = useState(APPROVED_SOURCE);
  const [compilerStatus, setCompilerStatus] = useState('idle');
  const [compilerDiagnostics, setCompilerDiagnostics] = useState<
    NativeReactCompileDiagnostic[]
  >([]);
  const diagnosticConsoleStore = useMemo(createStudioRunConsoleStore, []);
  const projection = useMemo(
    () =>
      createFrontstageJsxEditorProjection({
        catalogEntry: {
          ...builtinEntry,
          codeModules
        }
      }),
    []
  );
  const policy = useMemo(
    () =>
      validateNativeTrustedBlockSource(source, {
        allowedImportSources: projection.allowedImportSources
      }),
    [projection.allowedImportSources, source]
  );
  const policyDiagnostics: NativeReactCompileDiagnostic[] = policy.ok
    ? []
    : policy.errors.map(toCompileDiagnostic);
  const displayedDiagnostics =
    compilerDiagnostics.length > 0
      ? compilerDiagnostics
      : policyDiagnostics.length > 0
        ? policyDiagnostics
        : [runtimeDiagnostic];
  const firstLocation = policyDiagnostics[0]?.sourceLocation;

  const compile = async () => {
    setCompilerStatus('running');
    setCompilerDiagnostics([]);
    const result = await compileNativeReactComponentInBrowser({
      source,
      requestId: `r5-fixture:${source === APPROVED_SOURCE ? 'approved' : 'denied'}`,
      dependencyLock
    });
    setCompilerStatus(result.ok ? 'passed' : 'failed');
    setCompilerDiagnostics(result.diagnostics);
  };

  return (
    <ConfigProvider>
      <main className="r5-fixture-shell">
        <Typography.Title level={3}>R5 Studio Catalog</Typography.Title>
        <Space wrap>
          <Button
            onClick={() => {
              setSource(APPROVED_SOURCE);
              setCompilerStatus('idle');
              setCompilerDiagnostics([]);
            }}
          >
            Approved imports
          </Button>
          <Button
            onClick={() => {
              setSource(DENIED_SOURCE);
              setCompilerStatus('idle');
              setCompilerDiagnostics([]);
            }}
          >
            Denied import
          </Button>
          <Button type="primary" onClick={() => void compile()}>
            Compile current source
          </Button>
        </Space>

        <pre aria-label="Fixture source" className="r5-fixture-source">
          {source}
        </pre>
        <div
          data-testid="r5-studio-catalog-stats"
          data-compiler-status={compilerStatus}
          data-policy-errors={policyDiagnostics.length}
          data-marker-line={firstLocation?.line ?? 0}
          data-marker-column={firstLocation?.column ?? 0}
        />

        <section className="r5-fixture-run-panel">
          <JsxStudioPreviewConsole
            preview={<div data-testid="r5-preview-content">Preview ready</div>}
            snapshot={{
              diagnostics: displayedDiagnostics,
              apiCalls,
              consoleStore: diagnosticConsoleStore
            }}
          />
        </section>

        <section className="r5-fixture-run-panel">
          <JsxStudioRunPanel
            block={consoleBlock}
            code={consoleSource}
            revision="r7:browser-console"
          />
        </section>

      </main>
      <style>{`
        html, body, #root { min-height: 100%; margin: 0; }
        body { background: #f5f7f6; color: #16211d; font-family: Inter, system-ui, sans-serif; }
        .r5-fixture-shell { box-sizing: border-box; display: grid; gap: 16px; min-height: 100vh; padding: 20px; }
        .r5-fixture-source { box-sizing: border-box; max-height: 180px; margin: 0; padding: 12px; overflow: auto; border: 1px solid #d5ddd8; border-radius: 6px; background: #fff; white-space: pre-wrap; }
        .r5-fixture-run-panel { height: 460px; min-height: 0; border: 1px solid #d5ddd8; border-radius: 6px; overflow: hidden; background: #fff; }
        [data-testid='r5-preview-content'] { padding: 16px; }
        @media (max-width: 767px) {
          .r5-fixture-shell { padding: 12px; }
          .r5-fixture-run-panel { height: 520px; }
        }
      `}</style>
    </ConfigProvider>
  );
}

function toCompileDiagnostic(
  error: BlockProtocolError
): NativeReactCompileDiagnostic {
  return {
    phase: 'compile',
    code: error.code,
    path: error.path,
    message: error.message,
    ...(error.sourceLocation ? { sourceLocation: error.sourceLocation } : {})
  };
}

function createCatalogEntry(input: {
  id: string;
  title: string;
  installationId: string;
  providerCode: string;
  pluginId: string;
  pluginVersion: string;
  contributionCode: string;
  templateSource: string;
}): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: input.id,
    runtimeKind: 'native_react',
    installationId: input.installationId,
    providerCode: input.providerCode,
    pluginId: input.pluginId,
    pluginVersion: input.pluginVersion,
    contributionCode: input.contributionCode,
    title: input.title,
    entry: 'index.js',
    permissions: { network: 'none', storage: 'none', secrets: 'none' },
    contextContract: { primitives: [], inputSchema: {} },
    uiCapabilities: [],
    codeCapabilities: {
      template: {
        source: input.templateSource,
        version: input.pluginVersion,
        language: 'tsx'
      },
      allowedImports: [],
      monacoExtraLibs: []
    },
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw']
  };
}

await appI18n.changeLanguage('zh_Hans');
const root = document.getElementById('root');
if (!root) throw new Error('R5 Studio Catalog fixture root is missing.');
createRoot(root).render(<R5StudioCatalogFixture />);
