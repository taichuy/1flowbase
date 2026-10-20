import { describe, test, expect, vi } from 'vitest';
import {
  FrontstageJsxStudioDrawer,
  LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC,
  block,
  blockCodeHook,
  blockTabsHook,
  catalogEntry,
  domEvents,
  monacoEditor,
  monacoHook,
  renderSettled,
  screen,
  waitFor,
  within
} from './FrontstageJsxStudioDrawer.fixtures';

describe('FrontstageJsxStudioDrawer', () => {
  test('preserves JSX in Monaco because Page Runtime owns TSX compilation', async () => {
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

    expect(monacoHook.setCompilerOptions).toHaveBeenCalledWith(
      expect.objectContaining({ jsx: 'preserve' })
    );
  });

  test('R5-AC-001/002 uses Catalog imports for inline Monaco problems', async () => {
    const model = {};
    monacoEditor.getModel.mockReturnValue(model);
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: "import { Button } from '@1flowbase/native-components';",
      draft: "import { Button } from '@1flowbase/native-components';",
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save: vi.fn().mockResolvedValue(undefined)
    });
    const props = {
      open: true,
      initialSection: 'code' as const,
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      block,
      catalogEntry,
      onClose: vi.fn(),
      onSaveBlock: vi.fn()
    };
    const view = await renderSettled(<FrontstageJsxStudioDrawer {...props} />);

    expect(monacoHook.setModelMarkers).toHaveBeenLastCalledWith(
      model,
      expect.any(String),
      []
    );
    expect(screen.queryByText('代码诊断')).not.toBeInTheDocument();

    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: "import lodash from 'lodash';",
      draft: "import lodash from 'lodash';",
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save: vi.fn().mockResolvedValue(undefined)
    });
    view.rerender(<FrontstageJsxStudioDrawer {...props} />);

    await waitFor(() =>
      expect(monacoHook.setModelMarkers).toHaveBeenLastCalledWith(
        model,
        expect.any(String),
        [
          expect.objectContaining({
            code: 'import_denied',
            message: "Import source 'lodash' is not allowed.",
            startLineNumber: 1,
            startColumn: 1
          })
        ]
      )
    );
    expect(screen.queryByText('代码诊断')).not.toBeInTheDocument();
  });

  test('AC-005 submits snippet and import changes as one Monaco edit batch', async () => {
    const source = `export default function Block({ ctx }: NativeReactBlockProps) {
  return <div>content</div>;
}`;
    const selectionOffset = source.indexOf('content');
    const positionAt = (offset: number) => {
      const before = source.slice(0, offset).split('\n');
      return {
        lineNumber: before.length,
        column: (before.at(-1)?.length ?? 0) + 1
      };
    };
    const offsetAt = ({
      lineNumber,
      column
    }: {
      lineNumber: number;
      column: number;
    }) => {
      const lines = source.split('\n');
      return (
        lines
          .slice(0, lineNumber - 1)
          .reduce((total, line) => total + line.length + 1, 0) +
        column -
        1
      );
    };
    const selectionPosition = positionAt(selectionOffset);
    monacoEditor.getSelection.mockReturnValue({
      getStartPosition: () => selectionPosition,
      getEndPosition: () => selectionPosition
    });
    monacoEditor.getModel.mockReturnValue({
      getValue: () => source,
      getOffsetAt: offsetAt,
      getPositionAt: positionAt
    });
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: source,
      draft: source,
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save: vi.fn().mockResolvedValue(undefined)
    });

    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="components"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );
    const row = screen.getByText('Button').closest('tr');
    await domEvents.click(within(row!).getByRole('button', { name: '插入' }));

    await waitFor(() =>
      expect(monacoEditor.executeEdits).toHaveBeenCalledTimes(1)
    );
    expect(monacoEditor.executeEdits.mock.calls[0]?.[1]).toHaveLength(2);
    expect(monacoEditor.pushUndoStop).toHaveBeenCalledTimes(2);
  });

  test('AC-004 saves fixed block height from the configuration section', async () => {
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

    await domEvents.mouseDown(
      screen.getByRole('combobox', { name: '高度模式' })
    );
    await domEvents.click(await screen.findByText('固定高度'));
    await domEvents.change(
      screen.getByRole('spinbutton', { name: '固定高度' }),
      {
        target: { value: '360' }
      }
    );
    await domEvents.click(screen.getByRole('button', { name: '保存配置' }));

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onSaveBlock.mock.calls[0]?.[0]).toMatchObject({
      presentation: { heightMode: 'fixed', height: 360 }
    });
  });

  test('AC-003 does not save the host block title into runtime props', async () => {
    const onSaveBlock = vi.fn().mockResolvedValue(true);
    const onSaveBlockTitle = vi.fn().mockResolvedValue(true);
    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="configuration"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={{ ...block, props: {} }}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={onSaveBlock}
        onSaveBlockTitle={onSaveBlockTitle}
      />
    );

    await domEvents.change(screen.getByRole('textbox', { name: '标题' }), {
      target: { value: 'K7M2PX9Q' }
    });
    await domEvents.click(screen.getByRole('button', { name: '保存配置' }));

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onSaveBlock.mock.calls[0]?.[0].props).not.toHaveProperty('title');
    expect(onSaveBlockTitle).toHaveBeenCalledWith('orders-block', 'K7M2PX9Q');
  });

  test('AC-006 declares a typed output port from the variables section', async () => {
    const onSaveBlock = vi.fn().mockResolvedValue(true);
    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="variables"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        pageBlocks={[block]}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={onSaveBlock}
      />
    );
    await domEvents.change(screen.getByRole('textbox', { name: '输出端口' }), {
      target: { value: 'total' }
    });
    await domEvents.click(screen.getByRole('button', { name: '添加端口' }));
    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onSaveBlock.mock.calls[0]?.[0]).toMatchObject({
      ports: { outputs: [{ name: 'total', schema: { type: 'string' } }] }
    });
  });

  test('AC-043/044/046 runs from the header and keeps save persistence separate', async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const setDraft = vi.fn();
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: 'export default {}',
      draft: 'export default {}',
      dirty: true,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft,
      reset: vi.fn(),
      save
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
        runPanel={({ code, runRevision }) => (
          <div>{`Preview ${runRevision ?? 'idle'}: ${code}`}</div>
        )}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    const windowHeader = document.querySelector(
      '.frontstage-jsx-studio__window-header'
    );
    expect(windowHeader).not.toBeNull();
    expect(
      windowHeader?.querySelector('.frontstage-jsx-studio__window-actions')
    ).toHaveStyle({ flexWrap: 'wrap' });
    const headerButtons = within(windowHeader as HTMLElement).getAllByRole(
      'button'
    );
    expect(
      headerButtons
        .slice(0, 4)
        .map((button) => button.textContent?.replace(/\s+/gu, ''))
    ).toEqual(['上下文', '重置', '保存', '运行']);
    expect(
      within(windowHeader as HTMLElement).getByRole('button', {
        name: /^运\s*行$/
      })
    ).toHaveClass('ant-btn-primary');
    expect(
      within(windowHeader as HTMLElement).getByRole('button', {
        name: /保\s*存/
      })
    ).not.toHaveClass('ant-btn-primary');
    await domEvents.click(
      within(windowHeader as HTMLElement).getByRole('button', {
        name: '上下文'
      })
    );
    expect(setDraft).toHaveBeenCalledWith(
      expect.stringContaining('@1flowbase-context')
    );
    await domEvents.click(screen.getByRole('button', { name: /^运\s*行$/ }));
    expect(
      screen.getByText('Preview 1: export default {}')
    ).toBeInTheDocument();
    expect(save).not.toHaveBeenCalled();
    await domEvents.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  });

  test('does not expose migration, upgrade, or dependency state UI for legacy rows', async () => {
    const source = 'export default function Legacy() { return <div />; }';
    const saveActiveDraft = vi.fn().mockResolvedValue(undefined);
    const activeTab = {
      block_id: block.id,
      detail: { block_id: block.id, tab_id: 'tab-1', title: 'Orders' },
      base_source: source,
      draft: source,
      source_sha256: null,
      executable: {
        block_id: block.id,
        page_id: 'page-1',
        source_code: source,
        source_sha256: null,
        dependency_lock: null
      },
      loading: false,
      saving: false,
      error: null
    };
    blockTabsHook.useFrontstageBlockTabs.mockReturnValue({
      tabs: [activeTab],
      activeBlockId: activeTab.block_id,
      activeTab,
      anyDirty: false,
      openBlock: vi.fn(),
      activateBlock: vi.fn(),
      closeBlock: vi.fn(),
      setDraft: vi.fn(),
      setActiveDraft: vi.fn(),
      resetActive: vi.fn(),
      saveActiveDraft,
      handleDeletedBlock: vi.fn().mockResolvedValue('converged')
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

    expect(screen.queryByText(/迁移|升级|可执行依赖/u)).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    await domEvents.click(screen.getByRole('button', { name: /保\s*存/ }));
    expect(saveActiveDraft).not.toHaveBeenCalled();
  });

  test('D4-AC-006 preserves controlled legacy source and blocks save/run with the stable diagnostic', async () => {
    const legacySource = `async function main(ctx) { return { view: null, outputs: {} }; }
export default { main } satisfies BlockModule;`;
    const save = vi.fn();
    const runPanel = vi.fn(({ runRevision }) => (
      <div>{String(runRevision)}</div>
    ));
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: legacySource,
      draft: legacySource,
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save
    });
    const model = {};
    monacoEditor.getModel.mockReturnValue(model);

    await renderSettled(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        runPanel={runPanel}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    expect(screen.getByRole('textbox', { name: 'JSX source' })).toHaveValue(
      legacySource
    );
    expect(monacoHook.setModelMarkers).toHaveBeenLastCalledWith(
      model,
      expect.any(String),
      [
        expect.objectContaining({
          message: LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC.message
        })
      ]
    );
    expect(screen.queryByText('代码诊断')).not.toBeInTheDocument();
    await domEvents.click(screen.getByRole('button', { name: /^运\s*行$/ }));
    await domEvents.click(screen.getByRole('button', { name: /保\s*存/ }));
    expect(save).not.toHaveBeenCalled();
    expect(runPanel.mock.calls.at(-1)?.[0]).toMatchObject({
      code: legacySource,
      runRevision: null
    });
  });
});
