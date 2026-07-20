import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import copy from 'copy-to-clipboard';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const publicApi = vi.hoisted(() => ({
  applicationApiKeysQueryKey: vi.fn((applicationId: string) => [
    'applications',
    applicationId,
    'public-api',
    'keys'
  ]),
  applicationApiMappingQueryKey: vi.fn((applicationId: string) => [
    'applications',
    applicationId,
    'public-api',
    'mapping'
  ]),
  applicationOperationBindingsQueryKey: vi.fn((applicationId: string) => [
    'applications',
    applicationId,
    'public-api',
    'operation-bindings'
  ]),
  applicationApiPublicationQueryKey: vi.fn((applicationId: string) => [
    'applications',
    applicationId,
    'public-api',
    'publication'
  ]),
  applicationApiDocsCatalogQueryKey: vi.fn(),
  applicationApiDocsCategoryOperationsQueryKey: vi.fn(),
  applicationApiDocsOperationSpecQueryKey: vi.fn(),
  fetchApplicationApiKeys: vi.fn(),
  createApplicationApiKey: vi.fn(),
  revokeApplicationApiKey: vi.fn(),
  fetchApplicationApiMapping: vi.fn(),
  fetchApplicationOperationBindings: vi.fn(),
  saveApplicationApiMapping: vi.fn(),
  fetchApplicationApiPublication: vi.fn(),
  publishApplicationApiVersion: vi.fn(),
  unpublishApplicationApiVersion: vi.fn(),
  fetchApplicationApiDocsCatalog: vi.fn(),
  fetchApplicationApiDocsCategoryOperations: vi.fn(),
  fetchApplicationApiDocsOperationSpec: vi.fn(),
  getApplicationApiDocsLocale: vi.fn(() => null)
}));

vi.mock('../api/public-api', () => publicApi);
vi.mock('copy-to-clipboard', () => ({
  default: vi.fn()
}));
vi.mock('../../../shared/ui/api-docs/ApiDocsExplorer', () => ({
  ApiDocsExplorer: () => <div>docs explorer</div>
}));

import { AppProviders } from '../../../app/AppProviders';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import type { ApplicationDetail } from '../api/applications';
import { ApplicationApiKeysPanel } from '../components/api/ApplicationApiKeysPanel';
import { ApplicationApiPage } from '../pages/ApplicationApiPage';
import {
  editableOperationBindingsFixture,
  emptyOperationBindingOptionsFixture,
  readOnlyOperationBindingsFixture
} from './application-api-page/operation-bindings-fixtures';

const mapping = {
  input: {
    query_target: 'node-start.query',
    model_target: 'node-start.model',
    inputs_target: 'node-start',
    history_target: 'node-start.history',
    attachments_target: 'node-start.files'
  },
  output: {
    answer_selector: 'answer',
    usage_selector: null,
    files_selector: null,
    error_selector: null
  }
};

const application: ApplicationDetail = {
  id: 'app-1',
  application_type: 'agent_flow',
  workflow_trigger_type: null,
  name: 'Support Agent',
  description: 'customer support',
  icon: null,
  icon_type: null,
  icon_background: null,
  created_by: 'user-1',
  updated_at: '2026-05-09T00:00:00Z',
  tags: [],
  sections: {
    orchestration: {
      status: 'ready',
      subject_kind: 'agent_flow',
      subject_status: 'ready',
      current_subject_id: 'flow-1',
      current_draft_id: 'draft-1'
    },
    api: {
      status: 'active',
      credential_kind: 'application_api_key',
      invoke_routing_mode: 'api_key_bound_application',
      invoke_path_template: '/api/agent/v1/runs',
      api_capability_status: 'enabled',
      credentials_status: 'configured'
    },
    logs: {
      status: 'ready',
      runs_capability_status: 'enabled',
      run_object_kind: 'application_run',
      log_retention_status: 'enabled'
    },
    monitoring: {
      status: 'planned',
      metrics_capability_status: 'planned',
      metrics_object_kind: 'application_metrics',
      tracing_config_status: 'not_configured'
    }
  }
};

function renderWithProviders(ui: ReactNode) {
  return render(<AppProviders>{ui}</AppProviders>);
}

describe('ApplicationApiPage', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    vi.mocked(copy).mockResolvedValue(true);
    window.localStorage.clear();
    window.history.replaceState(null, '', '/?language=zh-Hans');
    await appI18n.changeLanguage('zh_Hans');
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: null
    });
    publicApi.fetchApplicationApiPublication.mockRejectedValue(
      new Error('application_not_published')
    );
    publicApi.fetchApplicationApiMapping.mockResolvedValue(mapping);
    publicApi.fetchApplicationOperationBindings.mockResolvedValue(
      editableOperationBindingsFixture()
    );
    publicApi.fetchApplicationApiKeys.mockResolvedValue([]);
  });

  afterEach(async () => {
    window.history.replaceState(null, '', '/');
    await appI18n.changeLanguage('en_US');
  });

  test('AC-005 draft state shows a single publish switch without a separate warning alert', async () => {
    renderWithProviders(<ApplicationApiPage application={application} />);

    const statusCard = await screen.findByRole('region', {
      name: '公开 API 状态'
    });
    const publishSwitch = within(statusCard).getByRole('switch');
    expect(publishSwitch).not.toBeChecked();

    // Single-switch mind model: the draft state is expressed by the switch,
    // not a duplicated warning alert with its own publish button.
    expect(screen.queryByText('请先发布公开 API')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '发布当前版本' })
    ).not.toBeInTheDocument();

    expect(
      screen.getByRole('button', { name: 'API 密钥' })
    ).toBeInTheDocument();
    expect(screen.getByText('docs explorer')).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'API 文档' })
    ).not.toBeInTheDocument();
  });

  test('AC-005 toggling the switch on publishes the current version', async () => {
    publicApi.publishApplicationApiVersion.mockResolvedValue({
      id: 'publication-1',
      version_sequence: 1,
      api_enabled: true,
      mapping_snapshot: mapping,
      created_at: '2026-05-09T00:00:00Z',
      updated_at: '2026-05-09T00:00:00Z'
    });

    renderWithProviders(<ApplicationApiPage application={application} />);

    const statusCard = await screen.findByRole('region', {
      name: '公开 API 状态'
    });
    fireEvent.click(within(statusCard).getByRole('switch'));

    await waitFor(() => {
      expect(publicApi.publishApplicationApiVersion).toHaveBeenCalledWith(
        'app-1',
        mapping,
        'csrf-123'
      );
    });
    expect(publicApi.unpublishApplicationApiVersion).not.toHaveBeenCalled();
  });

  test('AC-005 toggling the switch off reverts to draft after confirmation', async () => {
    publicApi.fetchApplicationApiPublication.mockResolvedValue({
      id: 'publication-1',
      version_sequence: 1,
      api_enabled: true,
      mapping_snapshot: mapping,
      created_at: '2026-05-09T00:00:00Z',
      updated_at: '2026-05-09T00:00:00Z'
    });
    publicApi.unpublishApplicationApiVersion.mockResolvedValue(undefined);

    renderWithProviders(<ApplicationApiPage application={application} />);

    const statusCard = await screen.findByRole('region', {
      name: '公开 API 状态'
    });
    const publishSwitch = within(statusCard).getByRole('switch');
    expect(publishSwitch).toBeChecked();

    fireEvent.click(publishSwitch);

    const dialog = await screen.findByRole('dialog');
    fireEvent.click(within(dialog).getByRole('button', { name: '退回草稿' }));

    await waitFor(() => {
      expect(publicApi.unpublishApplicationApiVersion).toHaveBeenCalledWith(
        'app-1',
        'csrf-123'
      );
    });
    expect(publicApi.publishApplicationApiVersion).not.toHaveBeenCalled();
  });

  test('does not duplicate endpoint summaries above the API docs panel', async () => {
    publicApi.fetchApplicationApiPublication.mockResolvedValue({
      id: 'publication-1',
      version_sequence: 1,
      api_enabled: true,
      mapping_snapshot: mapping,
      created_at: '2026-05-09T00:00:00Z',
      updated_at: '2026-05-09T00:00:00Z'
    });

    renderWithProviders(<ApplicationApiPage application={application} />);

    const statusCard = await screen.findByRole('region', {
      name: '公开 API 状态'
    });

    expect(within(statusCard).queryByText('Native')).not.toBeInTheDocument();
    expect(within(statusCard).queryByText('OpenAI')).not.toBeInTheDocument();
    expect(within(statusCard).queryByText('Anthropic')).not.toBeInTheDocument();
    expect(
      within(statusCard).queryByText('/api/agent/v1/runs')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'API 文档' })
    ).not.toBeInTheDocument();
    expect(screen.getByText('docs explorer')).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Native API' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'OpenAI Compatible' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Anthropic Compatible' })
    ).not.toBeInTheDocument();
  });

  test('keeps compatible connection settings inside API docs only', async () => {
    publicApi.fetchApplicationApiPublication.mockResolvedValue({
      id: 'publication-1',
      version_sequence: 1,
      api_enabled: true,
      mapping_snapshot: mapping,
      created_at: '2026-05-09T00:00:00Z',
      updated_at: '2026-05-09T00:00:00Z'
    });

    renderWithProviders(<ApplicationApiPage application={application} />);

    await screen.findByRole('region', {
      name: '公开 API 状态'
    });

    expect(
      screen.queryByRole('region', { name: '外部 Agent 接入' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('OpenAI 兼容')).not.toBeInTheDocument();
    expect(screen.queryByText('Anthropic 兼容')).not.toBeInTheDocument();
    expect(screen.queryByText('/v1/chat/completions')).not.toBeInTheDocument();
    expect(screen.queryByText('/v1/messages')).not.toBeInTheDocument();
  });

  test('opens API key list from the public API header action', async () => {
    publicApi.fetchApplicationApiPublication.mockResolvedValue({
      id: 'publication-1',
      version_sequence: 1,
      api_enabled: true,
      mapping_snapshot: mapping,
      created_at: '2026-05-09T00:00:00Z',
      updated_at: '2026-05-09T00:00:00Z'
    });
    publicApi.fetchApplicationApiKeys.mockResolvedValue([
      {
        id: 'key-1',
        name: 'Server key',
        token_prefix: 'sk-019e1a2b48',
        creator_user_id: 'user-1',
        enabled: true,
        expires_at: null,
        last_used_at: '2026-05-10T01:02:03Z',
        created_at: '2026-05-09T00:00:00Z',
        updated_at: '2026-05-09T00:00:00Z'
      }
    ]);

    renderWithProviders(<ApplicationApiPage application={application} />);

    const statusCard = await screen.findByRole('region', {
      name: '公开 API 状态'
    });

    expect(
      within(statusCard).getByRole('button', { name: 'API 密钥' })
    ).toBeInTheDocument();
    expect(within(statusCard).queryByText('API Keys')).not.toBeInTheDocument();
    expect(
      within(statusCard).queryByText('完整 token 只在创建后显示一次。')
    ).not.toBeInTheDocument();
    expect(within(statusCard).queryByRole('table')).not.toBeInTheDocument();
    expect(
      within(statusCard).queryByText('Server key')
    ).not.toBeInTheDocument();

    fireEvent.click(
      within(statusCard).getByRole('button', { name: 'API 密钥' })
    );

    const dialog = await screen.findByRole('dialog', { name: 'API Keys' });
    expect(within(dialog).getByText('Server key')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('密钥说明')).toBeInTheDocument();
    expect(within(dialog).getByText('sk-019e1a2b48****')).toBeInTheDocument();
    expect(within(dialog).queryByText('sk-0****2b48')).not.toBeInTheDocument();
    expect(within(dialog).getByText('2026/05/09 08:00:00')).toBeInTheDocument();
    expect(within(dialog).getByText('最后使用时间')).toBeInTheDocument();
    expect(within(dialog).getByText('2026/05/10 09:02:03')).toBeInTheDocument();
    expect(
      within(dialog).queryByText('2026-05-09T00:00:00Z')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByText('sk-019e1a2b48')).not.toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '创建 Key' })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '删除' })
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('button', { name: '复制' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'API Keys' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'API 文档' })
    ).not.toBeInTheDocument();
    expect(screen.getByText('docs explorer')).toBeInTheDocument();
  });

  test('B3 renders the server-owned binding projection read-only for a viewer', async () => {
    publicApi.fetchApplicationOperationBindings.mockResolvedValue(
      readOnlyOperationBindingsFixture()
    );

    renderWithProviders(<ApplicationApiPage application={application} />);

    fireEvent.click(await screen.findByRole('button', { name: '操作绑定' }));

    const dialog = await screen.findByRole('dialog', { name: '操作绑定' });
    const generateSelect = await within(dialog).findByRole('combobox', {
      name: '生成'
    });
    expect(within(dialog).getByText('草稿操作绑定')).toBeInTheDocument();
    expect(
      within(dialog).getByText('你可以查看该应用的操作绑定，但没有编辑权限。')
    ).toBeInTheDocument();
    expect(within(dialog).getByText('已发布操作绑定快照')).toBeInTheDocument();
    expect(within(dialog).getByText('Frozen generate')).toBeInTheDocument();
    expect(within(dialog).getByText('已支持')).toBeInTheDocument();
    expect(within(dialog).getAllByText('未绑定')).toHaveLength(2);
    expect(within(dialog).getByText('不支持')).toBeInTheDocument();
    expect(
      within(dialog).getByText('Provider 不支持此操作。')
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('button', { name: '保存操作绑定' })
    ).not.toBeInTheDocument();
    expect(generateSelect).toHaveAttribute('aria-disabled', 'true');
  });

  test('B3 saves only draft binding choices and leaves the published snapshot immutable', async () => {
    publicApi.saveApplicationApiMapping.mockResolvedValue({
      ...mapping,
      operation_bindings: {
        generate: { target_node_id: 'node-draft-generate-b' },
        count_tokens: null,
        compact: {
          responses_compact: null,
          responses_compaction_v2: null
        }
      }
    });

    renderWithProviders(<ApplicationApiPage application={application} />);

    fireEvent.click(await screen.findByRole('button', { name: '操作绑定' }));

    const dialog = await screen.findByRole('dialog', { name: '操作绑定' });
    fireEvent.mouseDown(
      await within(dialog).findByRole('combobox', { name: '生成' })
    );
    fireEvent.click(
      await screen.findByText('Draft generate B · node-draft-generate-b')
    );
    fireEvent.click(
      within(dialog).getByRole('button', { name: '保存操作绑定' })
    );

    await waitFor(() => {
      expect(publicApi.saveApplicationApiMapping).toHaveBeenCalledWith(
        'app-1',
        {
          ...mapping,
          operation_bindings: {
            generate: { target_node_id: 'node-draft-generate-b' },
            count_tokens: null,
            compact: {
              responses_compact: null,
              responses_compaction_v2: null
            }
          }
        },
        'csrf-123'
      );
    });
    expect(within(dialog).getByText('Frozen generate')).toBeInTheDocument();
    expect(
      within(dialog).getByText('publication-frozen-1')
    ).toBeInTheDocument();
  });

  test('B3 gives an empty draft options list a formal status instead of an empty editor', async () => {
    publicApi.fetchApplicationOperationBindings.mockResolvedValue(
      emptyOperationBindingOptionsFixture()
    );

    renderWithProviders(<ApplicationApiPage application={application} />);

    fireEvent.click(await screen.findByRole('button', { name: '操作绑定' }));

    const dialog = await screen.findByRole('dialog', { name: '操作绑定' });
    expect(
      await within(dialog).findByText('当前草稿没有可用的操作绑定。')
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('button', { name: '保存操作绑定' })
    ).not.toBeInTheDocument();
  });

  test('shows created token once without writing it to storage or URL', async () => {
    const storageSpy = vi.spyOn(Storage.prototype, 'setItem');
    publicApi.createApplicationApiKey.mockResolvedValue({
      id: 'key-1',
      name: 'Server key',
      token: 'sk-019e1a463b39-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789ABCD',
      token_prefix: 'sk-019e1a463b39',
      creator_user_id: 'user-1',
      enabled: true,
      expires_at: null,
      last_used_at: null,
      created_at: '2026-05-09T00:00:00Z',
      updated_at: '2026-05-09T00:00:00Z'
    });

    renderWithProviders(
      <ApplicationApiKeysPanel
        applicationId="app-1"
        csrfToken="csrf-123"
        onCreatedToken={vi.fn()}
      />
    );

    fireEvent.click(await screen.findByRole('button', { name: '创建 Key' }));
    fireEvent.change(screen.getByLabelText('Key 名称'), {
      target: { value: 'Server key' }
    });
    const createButtons = screen.getAllByRole('button', { name: /创\s*建/ });
    fireEvent.click(createButtons[createButtons.length - 1]);

    expect(
      await screen.findByText(
        'sk-019e1a463b39-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789ABCD'
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('textbox', { name: 'API Key' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByDisplayValue(
        'sk-019e1a463b39-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789ABCD'
      )
    ).not.toBeInTheDocument();
    expect(
      screen.getByText('完整 token 只在创建后显示一次。')
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '复制' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '复制' }));
    await waitFor(() => {
      expect(copy).toHaveBeenCalledWith(
        'sk-019e1a463b39-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789ABCD'
      );
    });
    expect(publicApi.createApplicationApiKey).toHaveBeenCalledWith(
      'app-1',
      'Server key',
      'csrf-123'
    );
    expect(storageSpy).not.toHaveBeenCalled();
    expect(window.location.href).not.toContain('sk-019e1a463b39');

    fireEvent.click(screen.getByRole('button', { name: /关\s*闭/ }));

    await waitFor(() => {
      expect(
        screen.queryByText(
          'sk-019e1a463b39-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789ABCD'
        )
      ).not.toBeInTheDocument();
    });
  });
});
