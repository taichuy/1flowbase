import { describe, test, expect, vi } from 'vitest';
import {
  antdAppMocks,
  block,
  blockCodeHook,
  blockTabsHook,
  catalogEntry,
  domEvents,
  interfaceCapabilitiesHook,
  monacoEditor,
  renderSettled,
  templateApi,
  templateAuth,
  uiTemplatesHook
} from './FrontstageJsxStudioDrawer.fixtures';
import { FrontstageJsxStudioDrawer } from '../../components/jsx-studio/FrontstageJsxStudioDrawer';
import { screen, waitFor, within } from '@testing-library/react';

describe('FrontstageJsxStudioDrawer', () => {
  test('AC-004/009 exposes block tree without the legacy child-container write rail', async () => {
    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        pageBlocks={[block]}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    expect(
      screen.queryByRole('button', { name: '子容器' })
    ).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '区块树' })).toBeInTheDocument();
  });

  test('keeps Monaco visible while configuration and interface resources share one Studio', async () => {
    const onSaveBlock = vi.fn().mockResolvedValue(true);

    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="configuration"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={onSaveBlock}
      />
    );

    expect(screen.getByRole('dialog', { name: 'TSX 编辑器' })).toHaveStyle({
      height: '680px'
    });
    expect(await screen.findByLabelText('JSX source')).toBeInTheDocument();
    expect(screen.getByLabelText('JSX source')).toHaveAttribute(
      'data-edit-context',
      'false'
    );
    expect(
      screen
        .getByLabelText('标题')
        .closest('.frontstage-jsx-studio__configuration-panel')
    ).not.toBeNull();
    expect(
      interfaceCapabilitiesHook.useFrontstageInterfaceCapabilities
    ).not.toHaveBeenCalled();

    await domEvents.click(screen.getByRole('button', { name: '接口' }));
    await domEvents.click(
      await screen.findByText(
        '/api/runtime/models/application_conversations/list'
      )
    );
    await domEvents.click(screen.getByRole('button', { name: '插入代码' }));

    await waitFor(() =>
      expect(antdAppMocks.success).toHaveBeenCalledWith('接口代码已插入')
    );
    expect(onSaveBlock).not.toHaveBeenCalled();
  });

  test('AC-001 attributes a Monaco change to the block identified by its model path', async () => {
    const setDraft = vi.fn();
    const rootTab = {
      block_id: 'root',
      detail: { block_id: 'root', tab_id: 'tab-1', title: 'Root' },
      base_source: 'root source',
      draft: 'root source',
      source_sha256: 'root-sha256',
      loading: false,
      saving: false,
      error: null
    };
    const childTab = {
      ...rootTab,
      block_id: 'child',
      detail: { block_id: 'child', tab_id: 'tab-1', title: 'Child' },
      base_source: '',
      draft: ''
    };
    blockTabsHook.useFrontstageBlockTabs.mockReturnValue({
      tabs: [rootTab, childTab],
      activeBlockId: 'child',
      activeTab: childTab,
      anyDirty: false,
      openBlock: vi.fn(),
      activateBlock: vi.fn(),
      closeBlock: vi.fn(),
      setDraft,
      setActiveDraft: vi.fn(),
      resetActive: vi.fn(),
      saveActiveDraft: vi.fn().mockResolvedValue(undefined),
      handleDeletedBlock: vi.fn().mockResolvedValue('converged')
    });
    monacoEditor.getModel.mockReturnValue({
      uri: {
        toString: () => 'file:///frontstage/page-1/blocks/child.tsx'
      }
    });

    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );
    await domEvents.change(
      screen.getByRole('textbox', { name: 'JSX source' }),
      {
        target: { value: 'child draft' }
      }
    );

    expect(setDraft).toHaveBeenCalledWith('child', 'child draft');
  });

  test('AC-001/002/003 keeps templates in their own resource section and replaces the whole draft', async () => {
    const setDraft = vi.fn();
    const save = vi.fn().mockResolvedValue(undefined);
    const templateSource = `export default function TemplateBlock() {
  return <section>Template</section>;
}`;
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: 'export default function CurrentBlock() { return null; }',
      draft: 'export default function CurrentBlock() { return null; }',
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft,
      reset: vi.fn(),
      save
    });
    uiTemplatesHook.useFrontstageUiTemplates.mockReturnValue({
      data: [
        {
          template_id: 'dashboard-template',
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block',
          name: 'Dashboard',
          source: templateSource,
          language: 'tsx',
          version: '2.1.0',
          is_official: false,
          is_default: true
        },
        {
          template_id: 'other-template',
          provider_code: 'other-provider',
          contribution_code: 'other-block',
          name: 'Other template',
          source: 'export default {}',
          language: 'tsx',
          version: '1.0.0',
          is_official: false,
          is_default: false
        }
      ],
      isLoading: false
    });

    const view = await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    const editorPanel = view.container.querySelector(
      '.frontstage-jsx-studio__editor-panel'
    );
    expect(
      within(editorPanel as HTMLElement).queryByText('代码模板')
    ).not.toBeInTheDocument();

    await domEvents.click(screen.getByRole('button', { name: '代码模板' }));
    expect(
      screen.getByRole('heading', { name: '代码模板' })
    ).toBeInTheDocument();
    expect(screen.getByText('Dashboard · 2.1.0 · 默认')).toBeInTheDocument();
    expect(screen.queryByText('Other template')).not.toBeInTheDocument();

    await domEvents.click(screen.getByText('Dashboard · 2.1.0 · 默认'));
    await domEvents.click(screen.getByRole('button', { name: '替换当前代码' }));
    expect(await screen.findAllByText('替换当前代码？')).not.toHaveLength(0);
    await domEvents.click(
      within(screen.getByRole('dialog', { name: '替换当前代码？' })).getByRole(
        'button',
        { name: /^替\s*换$/ }
      )
    );

    await waitFor(() => expect(setDraft).toHaveBeenCalledWith(templateSource));
    await waitFor(() =>
      expect(
        document.querySelector('.ant-modal-confirm')
      ).not.toBeInTheDocument()
    );
    expect(save).not.toHaveBeenCalled();
  });

  test('AC-001/002 saves the unsaved active draft as an unpublished template', async () => {
    const draft =
      'export default function Current() { return <div>Unsaved</div>; }';
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      ...blockCodeHook.useFrontstageBlockCode(),
      draft,
      dirty: true
    });
    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="templates"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );
    await domEvents.click(
      screen.getByRole('button', { name: '保存为页面模板' })
    );
    const dialog = screen.getByRole('dialog', { name: '保存为页面模板' });
    await domEvents.change(
      within(dialog).getByRole('textbox', { name: '模板名称' }),
      { target: { value: '  我的模板  ' } }
    );
    await domEvents.click(
      within(dialog).getByRole('button', { name: '保存模板草稿' })
    );
    await waitFor(() =>
      expect(templateApi.createSettingsUiTemplate).toHaveBeenCalledWith(
        {
          name: '我的模板',
          source: draft,
          language: 'tsx',
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block'
        },
        'csrf-test'
      )
    );
    await waitFor(() =>
      expect(
        screen.queryByRole('dialog', { name: '保存为页面模板' })
      ).not.toBeInTheDocument()
    );
    expect(antdAppMocks.success).toHaveBeenCalledWith(
      '模板草稿已保存，可在 UI 管理中发布后使用。'
    );
    expect(blockCodeHook.useFrontstageBlockCode().save).not.toHaveBeenCalled();
  });

  test('AC-003 validates the name and retains input after a rejected save', async () => {
    templateApi.createSettingsUiTemplate.mockRejectedValue({ status: 403 });
    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="templates"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );
    await domEvents.click(
      screen.getByRole('button', { name: '保存为页面模板' })
    );
    const dialog = screen.getByRole('dialog', { name: '保存为页面模板' });
    const input = within(dialog).getByRole('textbox', { name: '模板名称' });
    await domEvents.change(input, { target: { value: '   ' } });
    await domEvents.click(
      within(dialog).getByRole('button', { name: '保存模板草稿' })
    );
    expect(await screen.findByText('请输入模板名称')).toBeInTheDocument();
    expect(templateApi.createSettingsUiTemplate).not.toHaveBeenCalled();
    await domEvents.change(input, { target: { value: '保留这个名称' } });
    await domEvents.click(
      within(dialog).getByRole('button', { name: '保存模板草稿' })
    );
    expect(await screen.findByText('没有创建模板的权限')).toBeInTheDocument();
    expect(input).toHaveValue('保留这个名称');
    expect(dialog).toBeInTheDocument();
  });

  test('AC-004 disables template creation without management permission', async () => {
    templateAuth.me.permissions = [];
    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="templates"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );
    expect(
      screen.getByRole('button', { name: '保存为页面模板' })
    ).toBeDisabled();
  });

  test('resizes the resource panel horizontally without resizing the Studio window', async () => {
    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="interfaces"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    const dialog = screen.getByRole('dialog', { name: 'TSX 编辑器' });
    const workspace = dialog.querySelector<HTMLElement>(
      '.frontstage-jsx-studio__workspace'
    );
    const resizeHandle = screen.getByRole('separator', {
      name: '调整资源面板宽度'
    });

    expect(workspace?.style.getPropertyValue('--resource-panel-width')).toBe(
      '320px'
    );
    await domEvents.mouseDown(resizeHandle, { clientX: 500 });
    await domEvents.mouseMove(document, { clientX: 400 });
    await domEvents.mouseUp(document);
    expect(workspace?.style.getPropertyValue('--resource-panel-width')).toBe(
      '420px'
    );
    expect(dialog).toHaveStyle({ width: '1080px' });

    await domEvents.keyDown(resizeHandle, { key: 'ArrowRight' });
    expect(workspace?.style.getPropertyValue('--resource-panel-width')).toBe(
      '380px'
    );
  });
});
