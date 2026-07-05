import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { BlockCodeEditorDrawer } from '../components/BlockCodeEditorDrawer';
import type { FrontstageBlockInstance } from '../lib/page-document';

const blockCodeHook = vi.hoisted(() => ({
  useFrontstageBlockCode: vi.fn()
}));

vi.mock('../hooks/use-frontstage-block-code', () => blockCodeHook);

const block: FrontstageBlockInstance = {
  id: 'hero-block',
  sourceId: 'source-hero',
  codeRef: 'hero-code',
  sourceCodeRef: 'hero-code',
  catalog: {
    providerCode: 'acme',
    installationId: 'install-1'
  },
  contribution: {
    pluginId: 'plugin-1',
    pluginVersion: '1.0.0',
    code: 'hero'
  },
  props: {},
  layout: { order: 0 },
  order: 0,
  runtime: {
    kind: 'component',
    entry: 'Hero',
    hint: 'component'
  }
};

function mockBlockCodeState(
  state: Partial<ReturnType<typeof createBlockCodeState>> = {}
) {
  const nextState = {
    ...createBlockCodeState(),
    ...state
  };

  blockCodeHook.useFrontstageBlockCode.mockReturnValue(nextState);
  return nextState;
}

function createBlockCodeState() {
  return {
    code: 'export default function Hero() {}',
    draft: 'export default function Hero() {}',
    dirty: false,
    loading: false,
    saving: false,
    error: null as Error | null,
    permissionDenied: false,
    setDraft: vi.fn(),
    reset: vi.fn(),
    save: vi.fn().mockResolvedValue(undefined)
  };
}

describe('BlockCodeEditorDrawer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockBlockCodeState();
  });

  test('shows selected block metadata and loads code draft through the block code hook', async () => {
    render(
      <BlockCodeEditorDrawer
        open
        onClose={() => undefined}
        workspaceId="workspace-1"
        pageId="page-1"
        block={block}
      />
    );

    expect(await screen.findByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText('hero-block')).toBeInTheDocument();
    expect(screen.getByText(/hero-code/)).toBeInTheDocument();
    expect(screen.getByLabelText('Block code draft')).toHaveValue(
      'export default function Hero() {}'
    );
    expect(blockCodeHook.useFrontstageBlockCode).toHaveBeenCalledWith({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      codeRef: 'hero-code'
    });
  });

  test('marks dirty draft, resets it, and saves it through the hook', async () => {
    const state = mockBlockCodeState({
      draft: 'export default 2;',
      dirty: true
    });

    render(
      <BlockCodeEditorDrawer
        open
        onClose={() => undefined}
        workspaceId="workspace-1"
        pageId="page-1"
        codeRef="standalone-code"
      />
    );

    expect(screen.getByText('未保存')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Block code draft'), {
      target: { value: 'export default 3;' }
    });
    expect(state.setDraft).toHaveBeenCalledWith('export default 3;');

    fireEvent.click(screen.getByRole('button', { name: /重\s*置/ }));
    expect(state.reset).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() => {
      expect(state.save).toHaveBeenCalledTimes(1);
    });
    expect(blockCodeHook.useFrontstageBlockCode).toHaveBeenCalledWith({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      codeRef: 'standalone-code'
    });
  });

  test('disables editing when no block or codeRef is selected', async () => {
    render(
      <BlockCodeEditorDrawer
        open
        onClose={() => undefined}
        workspaceId="workspace-1"
        pageId="page-1"
      />
    );

    expect(screen.getByText('未选择区块')).toBeInTheDocument();
    expect(
      screen.getByText('请选择一个带 codeRef 的区块后再编辑代码。')
    ).toBeInTheDocument();
    expect(screen.getByLabelText('Block code draft')).toBeDisabled();
    expect(screen.getByRole('button', { name: /重\s*置/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /保\s*存/ })).toBeDisabled();
    expect(blockCodeHook.useFrontstageBlockCode).toHaveBeenCalledWith({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      codeRef: null
    });
  });

  test('disables editing when the selected block has no codeRef', () => {
    render(
      <BlockCodeEditorDrawer
        open
        onClose={() => undefined}
        workspaceId="workspace-1"
        pageId="page-1"
        block={{ ...block, codeRef: '' }}
      />
    );

    expect(
      screen.getByText('当前区块缺少 codeRef，无法加载或保存代码。')
    ).toBeInTheDocument();
    expect(screen.getByLabelText('Block code draft')).toBeDisabled();
    expect(screen.getByRole('button', { name: /保\s*存/ })).toBeDisabled();
  });

  test('shows loading and error states without enabling save', () => {
    mockBlockCodeState({
      loading: true,
      error: new Error('load failed')
    });

    render(
      <BlockCodeEditorDrawer
        open
        onClose={() => undefined}
        workspaceId="workspace-1"
        pageId="page-1"
        block={block}
      />
    );

    expect(screen.getByText('代码加载中')).toBeInTheDocument();
    expect(screen.getByText('load failed')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /保\s*存/ })).toBeDisabled();
  });

  test('shows permission denied for block code 403 without leaking raw error text', () => {
    mockBlockCodeState({
      error: new Error('raw block permission detail'),
      permissionDenied: true
    });

    render(
      <BlockCodeEditorDrawer
        open
        onClose={() => undefined}
        workspaceId="workspace-1"
        pageId="hidden-page"
        block={block}
      />
    );

    expect(screen.getByText('无权限访问')).toBeInTheDocument();
    expect(
      screen.queryByText('raw block permission detail')
    ).not.toBeInTheDocument();
  });
});
