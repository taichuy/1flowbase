import { describe, expect, test, vi } from 'vitest';

vi.mock('virtual:1flowbase-native-module-declarations', () => ({
  default: { version: 1, roots: {}, resources: {} }
}));
import {
  createFrontstageContextComment,
  createFrontstageJsxEditorProjection
} from '../../lib/jsx-studio/editor-projection';

describe('Frontstage JSX editor projection', () => {
  test('keeps the complete import allowlist independent of the persisted component catalog', () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: null
    });
    expect([...projection.allowedImportSources]).toEqual(
      expect.arrayContaining([
        'react',
        'react/jsx-runtime',
        'antd',
        'antd-img-crop',
        'antd-style',
        '@1flowbase/ui',
        '@1flowbase/block-sdk',
        '@1flowbase/native-components',
        '@ant-design/icons',
        '@1flowbase/charts',
        '@1flowbase/token-trend',
        '@1flowbase/rich-text',
        '@ant-design/x',
        '@ant-design/x-markdown',
        '@dnd-kit/core',
        '@dnd-kit/core/dist/index.js'
      ])
    );
    expect(projection.contextComment).toBe(createFrontstageContextComment());
    expect(projection.contextComment).not.toContain('ctx.data');
    expect(projection.monacoExtraLibs.map((lib) => lib.source)).toEqual([
      '@1flowbase/native-react-jsx',
      '@1flowbase/native-react-context'
    ]);
  });

  test('registers only demanded frontend facades initially and retains all new-import completions', async () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: null
    });
    const initial = await projection.declarationLoader.loadForSource(
      `import { useResponsive } from 'antd-style';`
    );
    expect(
      initial.find((lib) => lib.source === 'antd-style')?.content
    ).toContain(
      'export function useResponsive(): Partial<Record<ResponsiveKey, boolean>>;'
    );
    expect(initial.some((lib) => lib.source === '@ant-design/x-markdown')).toBe(
      false
    );
    const full = await projection.declarationLoader.loadAll();
    expect(full).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: '@1flowbase/native-components',
          filePath:
            'file:///node_modules/@1flowbase/native-components/index.d.ts'
        }),
        expect.objectContaining({ source: '@ant-design/x-markdown' })
      ])
    );
    expect(
      full.find((lib) => lib.source === '@dnd-kit/core/dist/index.js')?.content
    ).toContain("export * from '@dnd-kit/core'");
  });
});
