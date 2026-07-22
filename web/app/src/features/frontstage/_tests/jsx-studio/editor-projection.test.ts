import { describe, expect, test } from 'vitest';

import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import {
  createFrontstageContextComment,
  createFrontstageJsxEditorProjection
} from '../../lib/jsx-studio/editor-projection';

describe('Frontstage JSX editor projection', () => {
  test('AC-021 projects only catalog components and the editable context comment', () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: null
    });

    expect(projection).toEqual({
      components: [],
      contextComment: createFrontstageContextComment(),
      monacoExtraLibs: []
    });
    expect(projection.contextComment).toContain('@1flowbase-context');
    expect(projection.contextComment).not.toContain('interfaces:');
    expect(projection.contextComment).not.toContain('ctx.data');
  });

  test('AC-002 keeps each component attached to its catalog module source', () => {
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
        }
      } as NormalizedFrontstageBlockCatalogEntry
    });

    expect(projection.components).toEqual([
      {
        name: 'Button',
        moduleSource: '@1flowbase/block-renderer/antd-facade'
      },
      {
        name: 'Stack',
        moduleSource: '@1flowbase/block-renderer/antd-facade'
      }
    ]);
  });
});
