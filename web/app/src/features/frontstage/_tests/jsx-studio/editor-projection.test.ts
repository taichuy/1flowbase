import { describe, expect, test } from 'vitest';

import {
  createFrontstageContextComment,
  createFrontstageJsxEditorProjection
} from '../../lib/jsx-studio/editor-projection';

describe('Frontstage JSX editor projection', () => {
  test('WP-D4 does not infer import availability from the persisted component catalog', () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: null
    });

    expect([...projection.allowedImportSources]).toEqual([
      'react',
      'react/jsx-runtime',
      'antd',
      '@1flowbase/ui',
      '@1flowbase/block-sdk',
      '@1flowbase/native-components',
      '@ant-design/icons',
      '@1flowbase/charts',
      '@1flowbase/rich-text',
      '@ant-design/x',
      '@ant-design/x-markdown'
    ]);
    expect(projection.contextComment).toBe(createFrontstageContextComment());
    expect(projection.monacoExtraLibs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ source: '@1flowbase/native-react-jsx' }),
        expect.objectContaining({ source: '@1flowbase/native-react-context' })
      ])
    );
    expect(projection.monacoExtraLibs).toEqual(
      expect.arrayContaining([expect.objectContaining({ source: 'react' })])
    );
    expect(projection.contextComment).toContain('@1flowbase-context');
    expect(projection.contextComment).not.toContain('interfaces:');
    expect(projection.contextComment).not.toContain('ctx.data');
  });

  test('AC-002 projects frontend-owned module declarations into Monaco', () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: null
    });

    expect(projection.monacoExtraLibs).toContainEqual(
      expect.objectContaining({
        source: '@1flowbase/native-components',
        filePath: 'file:///node_modules/@1flowbase/native-components/index.d.ts'
      })
    );
    expect(projection.monacoExtraLibs).toContainEqual(
      expect.objectContaining({
        source: '@ant-design/x-markdown'
      })
    );
  });
});
