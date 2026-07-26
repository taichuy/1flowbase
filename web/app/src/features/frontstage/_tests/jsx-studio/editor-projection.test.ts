import { describe, expect, test } from 'vitest';

import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import {
  createFrontstageContextComment,
  createFrontstageJsxEditorProjection
} from '../../lib/jsx-studio/editor-projection';

describe('Frontstage JSX editor projection', () => {
  test('AC-021 projects only backend catalog scope and the editable context comment', () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: null
    });

    expect(projection.componentCatalogQuery).toBeNull();
    expect(projection.contextComment).toBe(createFrontstageContextComment());
    expect(projection.monacoExtraLibs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ source: 'react' }),
        expect.objectContaining({ source: '@1flowbase/native-react-context' })
      ])
    );
    expect(projection.contextComment).toContain('@1flowbase-context');
    expect(projection.contextComment).not.toContain('interfaces:');
    expect(projection.contextComment).not.toContain('ctx.data');
  });

  test('AC-001 does not infer component APIs from TypeScript declarations', () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: {
        codeCapabilities: {
          template: null,
          allowedImports: ['@1flowbase/block-renderer/antd-facade'],
          monacoExtraLibs: [
            {
              source: '@1flowbase/block-renderer/antd-facade',
              filePath:
                'file:///node_modules/@1flowbase/block-renderer/antd-facade/index.d.ts',
              content:
                "declare module '@1flowbase/block-renderer/antd-facade' { export const Button: unknown; export const Stack: unknown; }"
            }
          ],
          workerModuleSources: ['@1flowbase/block-renderer/antd-facade']
        },
        installationId: 'installation-1',
        contributionCode: 'frontstage.js-ui-block'
      } as NormalizedFrontstageBlockCatalogEntry
    });

    expect(projection.componentCatalogQuery).toEqual({
      installation_id: 'installation-1',
      contribution_code: 'frontstage.js-ui-block'
    });
    expect(projection.monacoExtraLibs.at(-1)?.content).toContain(
      'export const Button'
    );
  });
});
