import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';

import { FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS } from '../../lib/jsx-studio/native-react-editor-contract';

const legacyAuthorFragments = [
  'function main',
  'BlockModule',
  'BlockResult',
  'actionId',
  'formValues',
  'antd-facade'
];
const repoRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../../../..'
);

describe('Native React author contract inventory', () => {
  test('R5-AC-005 teaches standard React source from the backend catalog manifest', () => {
    const manifest = readFileSync(
      resolve(
        repoRoot,
        'api/plugins/capability-plugins/1flowbase/manifest.yaml'
      ),
      'utf8'
    );
    const monacoDeclarations = FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS.map(
      ({ content }) => content
    ).join('\n');

    for (const authorSurface of [manifest, monacoDeclarations]) {
      expect(authorSurface).toContain('React');
      for (const legacyFragment of legacyAuthorFragments) {
        expect(authorSurface).not.toContain(legacyFragment);
      }
    }
    expect(manifest).toContain('export default function ExampleBlock');
    expect(monacoDeclarations).toContain('interface NativeReactBlockProps');
  });

  test('R5-AC-005 has no frontend default catalog id or template registry', () => {
    const frontstagePage = readFileSync(
      resolve(
        repoRoot,
        'web/app/src/features/frontstage/pages/FrontStagePage.tsx'
      ),
      'utf8'
    );

    expect(frontstagePage).not.toContain('DEFAULT_JS_BLOCK_CATALOG_ENTRY_ID');
    expect(
      existsSync(
        resolve(
          repoRoot,
          'web/app/src/features/frontstage/lib/block-templates.ts'
        )
      )
    ).toBe(false);
  });

  test('D4-P4 has no production entrypoint for the retired code-block runtime', () => {
    const retiredEntrypoints = [
      'web/packages/page-runtime/src/js-block-worker-executor.ts',
      'web/packages/page-runtime/src/js-block-worker-runtime.ts',
      'web/packages/page-runtime/src/js-block-host-effect-bridge.ts',
      'web/packages/page-runtime/src/js-block-runtime/compiled-artifact.ts',
      'web/app/src/features/frontstage/components/RestrictedBlockRuntimePreview.tsx',
      'web/app/src/features/frontstage/hooks/use-frontstage-page-canvas-runtime-sessions.ts',
      'web/app/src/shared/code-block/default-js-block-runtime.worker.ts',
      'web/packages/antd-facade/src/index.ts'
    ];

    for (const retiredEntrypoint of retiredEntrypoints) {
      expect(existsSync(resolve(repoRoot, retiredEntrypoint))).toBe(false);
    }

    const currentSurfaces = [
      'web/packages/page-runtime/src/index.ts',
      'web/packages/block-sdk/src/index.ts',
      'web/app/src/features/frontstage/components/jsx-studio/JsxStudioRunPanel.tsx',
      'web/app/src/features/frontstage/components/PageCanvas.tsx',
      'web/app/src/features/auth/components/PublicAuthBlock.tsx'
    ].map((file) => readFileSync(resolve(repoRoot, file), 'utf8'));

    expect(currentSurfaces.join('\n')).not.toMatch(
      /(?:type|interface)\s+BlockModule|BlockResult|JsBlockWorker|RestrictedBlockRuntime|runtimeSessionEntries|createPublicAuthRunRequest|\bformValues\b/u
    );
  });

  test('R6-AC-001/003 keeps editor debug UI out of renderers and production hosts', () => {
    const renderer = readFileSync(
      resolve(
        repoRoot,
        'web/app/src/features/frontstage/lib/native-trusted-block-react-adapter.tsx'
      ),
      'utf8'
    );
    const productionHosts = [
      'web/app/src/features/frontstage/components/PageCanvas.tsx',
      'web/app/src/features/auth/components/PublicAuthBlock.tsx'
    ].map((file) => readFileSync(resolve(repoRoot, file), 'utf8'));
    const editorRun = readFileSync(
      resolve(
        repoRoot,
        'web/app/src/features/frontstage/components/jsx-studio/JsxStudioRunPanel.tsx'
      ),
      'utf8'
    );

    expect(renderer).not.toMatch(
      /JsxStudioPreviewConsole|prepareNativeReactSource|\bModal\b|\bretry\b/u
    );
    expect(productionHosts.join('\n')).not.toMatch(
      /JsxStudioRunPanel|JsxStudioPreviewConsole|js-block-console/u
    );
    expect(editorRun).toContain('JsxStudioPreviewConsole');
  });
});
