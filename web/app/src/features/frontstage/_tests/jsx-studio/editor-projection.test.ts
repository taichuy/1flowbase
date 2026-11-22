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

  test('injects on-demand fetched plugin module type declarations into Monaco extraLibs and allowed imports', async () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: null,
      typeDeclarations: [
        {
          installation_id: 'install-1',
          plugin_id: 'calendar@1.0.0',
          plugin_version: '1.0.0',
          source: '@acme/calendar',
          version: '1.0.0',
          binding: 'fetched',
          type_declarations:
            "declare module '@acme/calendar' { export function Calendar(): null; }\n"
        },
        {
          installation_id: 'install-1',
          plugin_id: 'calendar@1.0.0',
          plugin_version: '1.0.0',
          source: 'react',
          version: '19.2.5',
          binding: 'host',
          type_declarations: "declare module 'react' {}"
        }
      ]
    });

    expect(projection.allowedImportSources.has('@acme/calendar')).toBe(true);
    expect(projection.monacoExtraLibs).toContainEqual({
      source: '@acme/calendar',
      filePath: 'file:///node_modules/@acme/calendar/index.d.ts',
      content:
        "declare module '@acme/calendar' { export function Calendar(): null; }"
    });
    expect(
      projection.monacoExtraLibs.some((lib) => lib.source === 'react')
    ).toBe(false);

    const demanded = await projection.declarationLoader.loadForSource(
      `import { Calendar } from '@acme/calendar';`
    );
    expect(demanded).toContainEqual({
      source: '@acme/calendar',
      filePath: 'file:///node_modules/@acme/calendar/index.d.ts',
      content:
        "declare module '@acme/calendar' { export function Calendar(): null; }"
    });
  });
});
