import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, test } from 'vitest';

import { FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS } from '../../lib/jsx-studio/native-react-editor-contract';
import { createBlankJsBlockTemplateCode } from '../../lib/block-templates';

const legacyAuthorFragments = [
  'function main',
  'BlockModule',
  'BlockResult',
  'actionId',
  'formValues',
  'antd-facade'
];

describe('D4 Native React author contract inventory', () => {
  test('D4-AC-005 teaches only standard React default-export source', () => {
    const repoRoot = resolve(process.cwd(), '../..');
    const manifest = readFileSync(
      resolve(
        repoRoot,
        'api/plugins/capability-plugins/1flowbase/manifest.yaml'
      ),
      'utf8'
    );
    const builtInTemplate = createBlankJsBlockTemplateCode({
      blockId: 'inventory-block',
      codeRef: 'inventory-code',
      contributionCode: 'frontstage.js-ui-block'
    });
    const monacoDeclarations = FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS.map(
      ({ content }) => content
    ).join('\n');

    for (const authorSurface of [
      manifest,
      builtInTemplate,
      monacoDeclarations
    ]) {
      expect(authorSurface).toContain('React');
      for (const legacyFragment of legacyAuthorFragments) {
        expect(authorSurface).not.toContain(legacyFragment);
      }
    }
    expect(manifest).toContain('export default function ExampleBlock');
    expect(builtInTemplate).toContain('export default function Block');
    expect(monacoDeclarations).toContain('interface NativeReactBlockProps');
  });

  test('D4-P4 has no production entrypoint for the retired code-block runtime', () => {
    const repoRoot = resolve(process.cwd(), '../..');
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
      'web/app/src/features/frontstage/components/JsBlockTrialPanel.tsx',
      'web/app/src/features/frontstage/components/PageCanvas.tsx',
      'web/app/src/features/auth/components/public-auth-block-host.ts'
    ].map((file) => readFileSync(resolve(repoRoot, file), 'utf8'));

    expect(currentSurfaces.join('\n')).not.toMatch(
      /(?:type|interface)\s+BlockModule|BlockResult|JsBlockWorker|RestrictedBlockRuntime|runtimeSessionEntries|createPublicAuthRunRequest|\bformValues\b/u
    );
  });
});
