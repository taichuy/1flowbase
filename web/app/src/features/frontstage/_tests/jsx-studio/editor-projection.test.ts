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
    expect([...projection.allowedImportSources]).toEqual([]);
    expect(projection.contextComment).toBe(createFrontstageContextComment());
    expect(projection.monacoExtraLibs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ source: '@1flowbase/native-react-jsx' }),
        expect.objectContaining({ source: '@1flowbase/native-react-context' })
      ])
    );
    expect(projection.monacoExtraLibs).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ source: 'react' })])
    );
    expect(projection.contextComment).toContain('@1flowbase-context');
    expect(projection.contextComment).not.toContain('interfaces:');
    expect(projection.contextComment).not.toContain('ctx.data');
  });

  test('D2-AC-002 projects registered standard React declarations into Monaco', () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: {
        codeModules: [
          {
            source: '@1flowbase/native-components',
            version: '1.0.0',
            binding: 'fetched',
            assets: [
              {
                role: 'browser_module',
                media_type: 'text/javascript; charset=utf-8',
                sha256:
                  'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
              }
            ],
            exports: ['Surface'],
            type_declarations:
              "declare module '@1flowbase/native-components' { export interface SurfaceProps extends import('react').HTMLAttributes<HTMLElement> {} export const Surface: import('react').ComponentType<SurfaceProps>; }"
          }
        ],
        installationId: 'installation-1',
        contributionCode: 'frontstage.js-ui-block'
      } as NormalizedFrontstageBlockCatalogEntry
    });

    expect(projection.componentCatalogQuery).toEqual({
      installation_id: 'installation-1',
      contribution_code: 'frontstage.js-ui-block'
    });
    expect([...projection.allowedImportSources]).toEqual([
      '@1flowbase/native-components'
    ]);
    expect(projection.monacoExtraLibs.at(-1)?.content).toContain(
      "import('react').ComponentType<SurfaceProps>"
    );
    expect(projection.monacoExtraLibs.at(-1)).toMatchObject({
      source: '@1flowbase/native-components',
      filePath: 'file:///node_modules/@1flowbase/native-components/index.d.ts'
    });
  });
});
