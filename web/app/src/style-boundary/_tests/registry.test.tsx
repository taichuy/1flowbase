import { render, screen, waitFor } from '@testing-library/react';
import { Grid } from 'antd';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

vi.mock('@scalar/api-reference-react', () => ({
  ApiReferenceReact: () => <div data-testid="style-boundary-scalar">Scalar</div>
}));

import { AppProviders } from '../../app/AppProviders';
import { appI18n } from '../../shared/i18n/app-i18n';
import { renderReactFlowScene } from '../../test/renderers/render-react-flow-scene';
import { StyleBoundaryHarness } from '../StyleBoundaryHarness';
import { getRuntimeScene, getSceneIdsForFiles } from '../registry';

describe('style boundary registry', () => {
  beforeEach(async () => {
    window.history.replaceState(null, '', '/?language=zh-Hans');
    await appI18n.changeLanguage('zh_Hans');
  });

  afterEach(async () => {
    window.history.replaceState(null, '', '/');
    await appI18n.changeLanguage('en_US');
  });

  test('renders the account popup component scene and exposes scene metadata on window', async () => {
    const scene = getRuntimeScene('component.account-popup');

    render(
      <AppProviders>
        <StyleBoundaryHarness scene={scene} />
      </AppProviders>
    );

    expect(await screen.findByText('个人资料')).toBeInTheDocument();
    expect(window.__STYLE_BOUNDARY__?.scene.id).toBe('component.account-popup');
    expect(window.__STYLE_BOUNDARY__?.ready).toBe(true);
    expect(screen.getByTestId('style-boundary-scene')).toBeInTheDocument();
  });

  test('throws when a requested scene id is missing', () => {
    expect(() => getRuntimeScene('component.missing')).toThrow(
      /Unknown style boundary scene/u
    );
  });

  test('anchors the sign-in boundary to the product layout root', () => {
    expect(getRuntimeScene('page.sign-in').boundaryNodes).toContainEqual({
      id: 'sign-in-container',
      selector: '.auth-sign-in-page',
      propertyAssertions: [{ property: 'display', expected: 'flex' }]
    });
  });

  test('maps changed files to explicitly declared scenes', () => {
    expect(
      getSceneIdsForFiles(['web/app/src/features/home/pages/HomePage.tsx'])
    ).toEqual(['page.home']);
    expect(
      getSceneIdsForFiles(['web/app/src/app-shell/app-shell.css'])
    ).toEqual([
      'component.account-popup',
      'component.account-trigger',
      'page.home',
      'page.frontstage',
      'page.application-detail',
      'page.application-api',
      'page.embedded-apps',
      'page.templates',
      'page.settings-extension-center-agent-flow',
      'page.settings',
      'page.settings-i18n.desktop',
      'page.settings-i18n.mobile',
      'page.settings-system-runtime',
      'page.settings-applications',
      'page.settings-mcp-management',
      'page.settings-docs',
      'page.me'
    ]);
    expect(
      getSceneIdsForFiles([
        'web/app/src/shared/ui/section-page-layout/SectionPageLayout.tsx'
      ])
    ).toEqual([
      'page.frontstage',
      'page.application-detail',
      'page.application-api',
      'page.settings-extension-center-agent-flow',
      'page.settings',
      'page.settings-i18n.desktop',
      'page.settings-i18n.mobile',
      'page.settings-system-runtime',
      'page.settings-applications',
      'page.settings-mcp-management',
      'page.settings-docs',
      'page.me'
    ]);
    expect(
      getSceneIdsForFiles([
        'web/app/src/shared/ui/scrollable-surface/ScrollableSurface.tsx'
      ])
    ).toEqual([
      'page.settings',
      'page.settings-applications',
      'page.settings-mcp-management'
    ]);
    expect(
      getSceneIdsForFiles([
        'web/app/src/features/settings/components/settings-section-surface.css'
      ])
    ).toEqual([
      'page.settings-extension-center-agent-flow',
      'page.settings',
      'page.settings-i18n.desktop',
      'page.settings-i18n.mobile',
      'page.settings-system-runtime',
      'page.settings-applications',
      'page.settings-mcp-management',
      'page.settings-docs'
    ]);
    expect(
      getSceneIdsForFiles([
        'web/app/src/features/settings/pages/i18n-catalog/i18n-catalog-page.css'
      ])
    ).toEqual(['page.settings-i18n.desktop', 'page.settings-i18n.mobile']);
    expect(
      getSceneIdsForFiles([
        'web/app/src/features/settings/pages/i18n-catalog/I18nCatalogPage.tsx',
        'web/app/src/features/settings/pages/i18n-catalog/I18nCatalogEntryDrawer.tsx'
      ])
    ).toEqual(['page.settings-i18n.desktop', 'page.settings-i18n.mobile']);
    expect(
      getSceneIdsForFiles([
        'web/app/src/features/settings/components/application-management/ApplicationManagementPanel.tsx'
      ])
    ).toEqual(['page.settings-applications']);
    expect(
      getSceneIdsForFiles(['web/app/src/features/me/pages/me-page.css'])
    ).toEqual(['page.me']);
  });

  test('registers the translation catalog page readiness and responsive browse boundaries', () => {
    const desktopScene = getRuntimeScene('page.settings-i18n.desktop');
    const mobileScene = getRuntimeScene('page.settings-i18n.mobile');

    expect(desktopScene.viewport).toEqual({ width: 1280, height: 800 });
    expect(desktopScene.boundaryNodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'catalog-ready',
          selector: '[data-testid="i18n-catalog-page"][data-ready="true"]'
        }),
        expect.objectContaining({ id: 'catalog-desktop-browse' })
      ])
    );
    expect(
      desktopScene.boundaryNodes.some(
        (node) => node.id === 'catalog-mobile-browse'
      )
    ).toBe(false);

    expect(mobileScene.viewport).toEqual({ width: 390, height: 844 });
    expect(mobileScene.boundaryNodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'catalog-ready',
          selector: '[data-testid="i18n-catalog-page"][data-ready="true"]'
        }),
        expect.objectContaining({ id: 'catalog-mobile-browse' })
      ])
    );
    expect(
      mobileScene.boundaryNodes.some(
        (node) => node.id === 'catalog-desktop-browse'
      )
    ).toBe(false);
  });

  test('renders the home page scene inside the shared shell frame', async () => {
    const scene = getRuntimeScene('page.home');

    render(
      <AppProviders>
        <StyleBoundaryHarness scene={scene} />
      </AppProviders>
    );

    expect(
      await screen.findByRole('heading', { name: '1flowbase' })
    ).toBeInTheDocument();
    expect(await screen.findByText('Support Agent')).toBeInTheDocument();
    expect(
      screen.getByRole('navigation', { name: 'Primary' })
    ).toBeInTheDocument();
  }, 15_000);

  test('renders the frontstage page scene with the shared section layout', async () => {
    const scene = getRuntimeScene('page.frontstage');

    render(
      <AppProviders>
        <StyleBoundaryHarness scene={scene} />
      </AppProviders>
    );

    expect(
      await screen.findByRole('heading', { name: '1flowbase' })
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('heading', { name: 'Landing' })
    ).toBeInTheDocument();
    expect(screen.getByLabelText('进入设计模式')).toBeInTheDocument();
  }, 15_000);

  test('application detail scene save mock echoes the latest draft document', async () => {
    const scene = getRuntimeScene('page.application-detail');

    renderReactFlowScene(<StyleBoundaryHarness scene={scene} />);

    await screen.findByTestId(
      'agent-flow-editor-body',
      {},
      { timeout: 15_000 }
    );

    const baseDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const nextDocument = {
      ...baseDocument,
      editor: {
        ...baseDocument.editor,
        viewport: { x: 120, y: 48, zoom: 0.85 }
      }
    };
    const saveResponse = await fetch(
      'http://127.0.0.1:7800/api/console/applications/app-1/orchestration/draft',
      {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          document: nextDocument,
          change_kind: 'layout',
          summary: '更新画布布局'
        })
      }
    );
    const savePayload = await saveResponse.json();
    const latestResponse = await fetch(
      'http://127.0.0.1:7800/api/console/applications/app-1/orchestration'
    );
    const latestPayload = await latestResponse.json();

    expect(savePayload.data.draft.document).toEqual(nextDocument);
    expect(latestPayload.data.draft.document).toEqual(nextDocument);
  }, 15_000);

  test('application detail scene reaches the editor shell instead of the error state', async () => {
    const scene = getRuntimeScene('page.application-detail');
    vi.spyOn(Grid, 'useBreakpoint').mockReturnValue({ lg: true } as never);
    const consoleErrorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const consoleWarnSpy = vi
      .spyOn(console, 'warn')
      .mockImplementation(() => undefined);

    renderReactFlowScene(<StyleBoundaryHarness scene={scene} />);

    await waitFor(
      () => {
        expect(
          screen.getByTestId('agent-flow-editor-body')
        ).toBeInTheDocument();
        expect(
          screen.getByRole('button', { name: '历史版本' })
        ).toBeInTheDocument();
      },
      { timeout: 15_000 }
    );
    expect(screen.queryByText('编排加载失败')).not.toBeInTheDocument();
    expect(
      [...consoleErrorSpy.mock.calls, ...consoleWarnSpy.mock.calls]
        .flat()
        .join('\n')
    ).not.toContain('[React Flow]');
  }, 20_000);

  test('renders the settings scene with canonical multi-provider contract data', async () => {
    const scene = getRuntimeScene('page.settings');

    render(
      <AppProviders>
        <StyleBoundaryHarness scene={scene} />
      </AppProviders>
    );

    expect(
      await screen.findByRole(
        'heading',
        { name: '模型供应商', level: 5 },
        { timeout: 5000 }
      )
    ).toBeInTheDocument();
    expect(await screen.findByText('已安装供应商')).toBeInTheDocument();
    expect(
      await screen.findByRole(
        'heading',
        { name: '模型供应商', level: 5 },
        { timeout: 5000 }
      )
    ).toBeInTheDocument();
    expect(
      (await screen.findAllByText('OpenAI Compatible')).length
    ).toBeGreaterThan(0);
    expect(await screen.findByText('Anthropic Compatible')).toBeInTheDocument();
    expect(
      await screen.findByRole(
        'button',
        { name: '当前已是最新版本' },
        { timeout: 5000 }
      )
    ).toBeInTheDocument();
  }, 15000);

  test('renders the settings applications scene with its management table', async () => {
    const scene = getRuntimeScene('page.settings-applications');

    render(
      <AppProviders>
        <StyleBoundaryHarness scene={scene} />
      </AppProviders>
    );

    expect(
      await screen.findByText('Boundary Workflow', undefined, {
        timeout: 10_000
      })
    ).toBeInTheDocument();
    expect(screen.getByText('Style boundary application')).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '名称' })
    ).toBeInTheDocument();
  }, 30_000);

  test('renders the settings mcp management scene', async () => {
    const scene = getRuntimeScene('page.settings-mcp-management');

    render(
      <AppProviders>
        <StyleBoundaryHarness scene={scene} />
      </AppProviders>
    );

    expect(await screen.findByText('Workspace Ops')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'MCP 实例' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Tool 配置' })).toBeInTheDocument();
  }, 15_000);

  test('renders the settings API docs scene from the backend navigation fixture', async () => {
    const scene = getRuntimeScene('page.settings-docs');
    const view = render(
      <AppProviders>
        <StyleBoundaryHarness scene={scene} />
      </AppProviders>
    );

    await waitFor(() => {
      expect(
        view.container.querySelector('.api-docs-panel')
      ).toBeInTheDocument();
    });
  }, 15_000);

  test('seeds model provider instances with enabled model ids instead of validation history', async () => {
    const scene = getRuntimeScene('page.settings');

    render(
      <AppProviders>
        <StyleBoundaryHarness scene={scene} />
      </AppProviders>
    );

    expect(
      await screen.findByRole(
        'heading',
        { name: '模型供应商', level: 5 },
        { timeout: 5000 }
      )
    ).toBeInTheDocument();

    const response = await fetch(
      'http://127.0.0.1:7800/api/console/settings/model-providers/instances'
    );
    const payload = await response.json();
    const instance = payload.data[0] as Record<string, unknown>;

    expect(instance).toEqual(
      expect.objectContaining({
        enabled_model_ids: expect.arrayContaining(['gpt-4o-mini']),
        model_count: expect.any(Number)
      })
    );
    expect(instance).not.toHaveProperty('validation_model_id');
    expect(instance).not.toHaveProperty('last_validated_at');
    expect(instance).not.toHaveProperty('last_validation_status');
    expect(instance).not.toHaveProperty('last_validation_message');
  }, 15000);
});
