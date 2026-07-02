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
  saveApplicationApiMapping: vi.fn(),
  fetchApplicationApiPublication: vi.fn(),
  publishApplicationApiVersion: vi.fn(),
  setApplicationApiEnabled: vi.fn(),
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
      status: 'configured',
      credential_kind: 'application_api_key',
      invoke_routing_mode: 'api_key_bound_application',
      invoke_path_template: '/api/agent/v1/runs',
      api_capability_status: 'enabled',
      credentials_status: 'active'
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
    publicApi.fetchApplicationApiKeys.mockResolvedValue([]);
  });

  afterEach(async () => {
    window.history.replaceState(null, '', '/');
    await appI18n.changeLanguage('en_US');
  });

  test('renders API section shell with status, docs panel, and publish action', async () => {
    renderWithProviders(<ApplicationApiPage application={application} />);

    expect(await screen.findByText('请先发布公开 API')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'API 密钥' })
    ).toBeInTheDocument();
    expect(screen.queryByText('API Keys')).not.toBeInTheDocument();
    expect(
      screen.queryByText('完整 token 只在创建后显示一次。')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '创建 Key' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'API Keys' })
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
    expect(
      screen.queryByRole('tab', { name: 'Mapping' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Debug' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '发布当前版本' })
    ).toBeInTheDocument();
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
