import '../../../../test/fixtures/rc-util-unique-ids';
import type { ConsoleCustomFrontendBlock } from '@1flowbase/api-client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import { I18nextProvider } from 'react-i18next';
import { beforeEach, expect, test, vi } from 'vitest';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { RegisteredBlocksTab } from '../../components/ui-management/RegisteredBlocksTab';

const api = vi.hoisted(() => ({
  fetchSettingsUiBlocks: vi.fn(),
  fetchSettingsCustomBlocks: vi.fn(),
  createSettingsCustomBlock: vi.fn(),
  updateSettingsCustomBlock: vi.fn(),
  deleteSettingsCustomBlock: vi.fn()
}));
vi.mock('../../api/ui-management', async (original) => ({
  ...(await original<typeof import('../../api/ui-management')>()),
  ...api
}));
vi.mock('../../../frontstage/hooks/use-frontstage-block-catalog', () => ({
  useFrontstageBlockCatalog: () => ({ items: [], loading: false })
}));
vi.mock(
  '../../../frontstage/components/jsx-studio/JsxStudioResourcePanel',
  () => ({
    JsxStudioResourcePanel: () => null
  })
);
vi.mock('../../../frontstage/components/jsx-studio/JsxStudioRunPanel', () => ({
  JsxStudioRunPanel: () => null
}));
// Exercise the real registered-block owner and studio draft; only replace the
// Monaco/window surface. Close represents the user's confirmed discard action.
vi.mock('../../../../shared/code-block/BlockSourceStudio', () => ({
  BlockSourceStudio: (props: {
    source: string;
    saving: boolean;
    onChange: (source: string) => void;
    onSave: () => void;
    onClose: () => void;
    renderResource: (section: 'configuration') => React.ReactNode;
  }) => (
    <dialog open aria-label="Block source studio">
      {props.renderResource('configuration')}
      <label>
        Block source
        <textarea
          value={props.source}
          onChange={(event) => props.onChange(event.target.value)}
        />
      </label>
      <button disabled={props.saving} onClick={props.onSave}>
        Save studio
      </button>
      <button onClick={props.onClose}>Close studio</button>
    </dialog>
  )
}));

const block: ConsoleCustomFrontendBlock = {
  id: 'block-a',
  workspace_id: 'workspace-1',
  provider_code: '1flowbase',
  contribution_code: 'frontstage.js-ui-block',
  title: 'First block',
  source: 'export default function First() { return null; }',
  language: 'tsx',
  revision: 1
};

beforeEach(async () => {
  vi.clearAllMocks();
  resetAuthStore();
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-token',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: null
  });
  await loadApplicationI18nResources();
  await appI18n.changeLanguage('en_US');
  api.fetchSettingsUiBlocks.mockResolvedValue([]);
  api.fetchSettingsCustomBlocks.mockResolvedValue([
    block,
    { ...block, id: 'block-b', title: 'Second block' }
  ]);
});

test('late save completion cannot close a newer registered block draft', async () => {
  let finishPrevious!: (value: ConsoleCustomFrontendBlock) => void;
  api.updateSettingsCustomBlock.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finishPrevious = resolve;
      })
  );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  const frontstageCatalogKey = ['frontstage', 'block-catalog', 'workspace-1'];
  client.setQueryData(frontstageCatalogKey, []);
  const view = render(
    <I18nextProvider i18n={appI18n}>
      <QueryClientProvider client={client}>
        <App>
          <RegisteredBlocksTab />
        </App>
      </QueryClientProvider>
    </I18nextProvider>
  );
  try {
    const firstRow = (await screen.findByText('First block')).closest('tr')!;
    fireEvent.click(within(firstRow).getByRole('button', { name: 'Edit' }));
    const previousStudio = await screen.findByRole(
      'dialog',
      { name: 'Block source studio' },
      { timeout: 10_000 }
    );
    fireEvent.change(within(previousStudio).getByLabelText('Name'), {
      target: { value: 'Saved first block' }
    });
    fireEvent.click(
      within(previousStudio).getByRole('button', { name: 'Save studio' })
    );
    await waitFor(() =>
      expect(api.updateSettingsCustomBlock).toHaveBeenCalledWith(
        'block-a',
        { title: 'Saved first block', source: block.source, language: 'tsx' },
        'csrf-token'
      )
    );
    fireEvent.click(
      within(previousStudio).getByRole('button', { name: 'Close studio' })
    );
    const secondRow = screen.getByText('Second block').closest('tr')!;
    fireEvent.click(within(secondRow).getByRole('button', { name: 'Edit' }));
    const nextStudio = await screen.findByRole('dialog', {
      name: 'Block source studio'
    });
    fireEvent.change(within(nextStudio).getByLabelText('Name'), {
      target: { value: 'New unsaved block' }
    });
    fireEvent.change(within(nextStudio).getByLabelText('Block source'), {
      target: { value: 'export default function Unsaved() { return null; }' }
    });
    await act(async () =>
      finishPrevious({ ...block, title: 'Saved first block', revision: 2 })
    );
    await waitFor(() => expect(client.isMutating()).toBe(0));
    const preservedStudio = screen.getByRole('dialog', {
      name: 'Block source studio'
    });
    expect(within(preservedStudio).getByLabelText('Name')).toHaveValue(
      'New unsaved block'
    );
    expect(within(preservedStudio).getByLabelText('Block source')).toHaveValue(
      'export default function Unsaved() { return null; }'
    );
    expect(api.updateSettingsCustomBlock).toHaveBeenCalledTimes(1);
    expect(api.fetchSettingsCustomBlocks).toHaveBeenCalledTimes(2);
    expect(api.fetchSettingsUiBlocks).toHaveBeenCalledTimes(2);
    expect(client.getQueryState(frontstageCatalogKey)?.isInvalidated).toBe(
      true
    );
  } finally {
    view.unmount();
    client.clear();
  }
});
