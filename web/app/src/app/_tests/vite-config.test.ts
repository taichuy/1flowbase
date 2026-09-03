import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { describe, expect, test } from 'vitest';
import appPackageJson from '../../../package.json';
import richTextPackageJson from '../../../../packages/rich-text/package.json';

describe('vite config', () => {
  test('AC-001 AC-002 keeps local dev bundleless and exposes stable remote preview', async () => {
    expect(appPackageJson.scripts).toEqual(
      expect.objectContaining({
        dev: 'vite',
        'dev:remote':
          'vite build --mode remote-debug && vite preview --mode remote-debug --host 0.0.0.0 --port 3100 --strictPort',
        'dev:remote:experimental': 'vite --experimentalBundle'
      })
    );

    const source = await readFile(
      path.resolve(process.cwd(), 'vite.config.ts'),
      'utf8'
    );

    expect(source).toContain("const isRemoteDebug = mode === 'remote-debug'");
    expect(source).toContain('sourcemap: isRemoteDebug');
  });

  test('AC-001 resolves Vditor and its rich-text runtime assets from one exact version', () => {
    expect(appPackageJson.dependencies.vditor).toBe('3.11.2');
    expect(richTextPackageJson.dependencies.vditor).toBe(
      appPackageJson.dependencies.vditor
    );
  });

  test('proxies backend routes to support same-origin docs requests', async () => {
    const source = await readFile(
      path.resolve(process.cwd(), 'vite.config.ts'),
      'utf8'
    );

    expect(source).toContain('VITE_API_PROXY_TARGET');
    expect(source).toContain("'/api'");
    expect(source).toContain("'/v1'");
    expect(source).toContain("'/health'");
    expect(source).toContain("'/openapi.json'");
    expect(source).toContain('target: apiProxyTarget');
    expect(source.match(/ws: true/gu)).toHaveLength(2);
  });

  test('AC-004 proxies the fixed external npm path in development', async () => {
    const source = await readFile(
      path.resolve(process.cwd(), 'vite.config.ts'),
      'utf8'
    );

    expect(source).toContain('VITE_EXTERNAL_NPM_PROXY_TARGET');
    expect(source).toContain("'/external-npm'");
    expect(source).toContain('target: externalNpmProxyTarget');
  });

  test('AC-004 keeps production external npm misses out of the SPA fallback', async () => {
    const source = await readFile(
      path.resolve(process.cwd(), '../../docker/web/nginx.conf'),
      'utf8'
    );

    expect(source).toContain('location = /external-npm/manifest.json');
    expect(source).toContain('location /external-npm/assets/');
    expect(source.match(/try_files \$uri =404;/gu)).toHaveLength(3);
  });

  test('can expose the dev proxy to configured frontend origins', async () => {
    const source = await readFile(
      path.resolve(process.cwd(), 'vite.config.ts'),
      'utf8'
    );

    expect(source).toContain('VITE_DEV_CORS_ALLOWED_ORIGINS');
    expect(source).toContain('devCorsAllowedOrigins');
    expect(source).toContain('credentials: true');
    expect(source).toContain('origin: devCorsAllowedOrigins');
  });

  test('keeps heavyweight route pages behind dynamic imports', async () => {
    const source = await readFile(
      path.resolve(process.cwd(), 'src/app/router.tsx'),
      'utf8'
    );

    expect(source).toContain(
      "import('../features/applications/pages/ApplicationDetailPage')"
    );
    expect(source).toContain('loadApplicationDetailPage()');
    expect(source).toMatch(
      /lazy\(\(\) =>\s+import\('\.\.\/features\/settings\/pages\/SettingsPage'\)/
    );
    expect(source).not.toContain(
      "import { ApplicationDetailPage } from '../features/applications/pages/ApplicationDetailPage'"
    );
    expect(source).not.toContain(
      "import { SettingsPage } from '../features/settings/pages/SettingsPage'"
    );
  });

  test('MDP-007 BGP-001 delegates resources to deterministic planners', async () => {
    const source = await readFile(
      path.resolve(process.cwd(), 'vite.config.ts'),
      'utf8'
    );

    expect(source).toContain('pageTreeIconAssetsPlugin');
    expect(source).toContain('manualChunks: planScenarioChunk');
    expect(source).toContain('scenarioChunkManifestPlugin');
    expect(source).toContain('chunkSizeWarningLimit: 3500');
  });

  test('pre-optimizes dependencies used by lazy application pages', async () => {
    const source = await readFile(
      path.resolve(process.cwd(), 'vite.config.ts'),
      'utf8'
    );
    const lazyOnlyDeps = [
      '@ant-design/x-markdown',
      '@lexical/react/LexicalComposer',
      '@lexical/react/LexicalComposerContext',
      '@lexical/react/LexicalContentEditable',
      '@lexical/react/LexicalErrorBoundary',
      '@lexical/react/LexicalHistoryPlugin',
      '@lexical/react/LexicalOnChangePlugin',
      '@lexical/react/LexicalRichTextPlugin',
      '@lexical/react/useLexicalNodeSelection',
      '@lexical/utils',
      '@monaco-editor/react',
      '@scalar/api-reference-react',
      '@xyflow/react',
      'copy-to-clipboard',
      'echarts',
      'lexical',
      'monaco-editor',
      'vditor'
    ];

    expect(source).toContain('optimizeDeps');
    expect(source).toContain("'@ant-design/icons'");
    expect(source).toContain("'@ant-design/icons-svg'");
    expect(source).toContain("command === 'serve'");
    expect(source).toContain("'@ant-design/icons-svg/es/asn'");
    expect(source).not.toContain(
      '...nativeAntDesignIconsModuleInventory.modules.map'
    );
    for (const dependency of lazyOnlyDeps) {
      expect(source).toContain(`'${dependency}'`);
    }
  });

  test('AC-002 leaves local ESM workspace packages out of dependency pre-optimization', async () => {
    const source = await readFile(
      path.resolve(process.cwd(), 'vite.config.ts'),
      'utf8'
    );

    const optimizeInclude = source.match(
      /include:\s*\[([\s\S]*?)\],\s*needsInterop/u
    )?.[1];
    expect(optimizeInclude).toBeDefined();
    expect(optimizeInclude).not.toContain("'@1flowbase/api-client/auth'");
  });

  test('replaces the react-draggable debug process lookup in dev and production bundles', async () => {
    const source = await readFile(
      path.resolve(process.cwd(), 'vite.config.ts'),
      'utf8'
    );

    expect(source).toContain('const reactDraggableBrowserDefines');
    expect(source).toContain("'process.env.DRAGGABLE_DEBUG': 'false'");
    expect(source.match(/define: reactDraggableBrowserDefines/g)).toHaveLength(
      1
    );
  });
});
