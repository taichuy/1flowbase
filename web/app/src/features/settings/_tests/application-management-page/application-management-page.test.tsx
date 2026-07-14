import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const applicationManagementApi = vi.hoisted(() => ({
  settingsApplicationManagementQueryKey: vi.fn((query: unknown) => [
    'settings',
    'applications',
    query
  ]),
  fetchSettingsApplicationManagement: vi.fn()
}));

const applicationsApi = vi.hoisted(() => ({
  applicationCatalogQueryKey: ['applications', 'catalog'],
  applicationsQueryKey: ['applications'],
  fetchApplicationCatalog: vi.fn(),
  updateApplication: vi.fn(),
  deleteApplication: vi.fn(),
  createApplication: vi.fn(),
  createApplicationTag: vi.fn(),
  exportAgentFlowTemplate: vi.fn()
}));

vi.mock('../../api/application-management', () => applicationManagementApi);
vi.mock('../../../applications/api/applications', () => applicationsApi);

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { ApplicationManagementPanel } from '../../components/application-management/ApplicationManagementPanel';

describe('ApplicationManagementPanel', () => {
  beforeEach(() => {
    resetAuthStore();
    window.history.replaceState(
      {},
      '',
      '/settings/applications?page=2&application_type=workflow&publication_status=unpublished&keyword=Daily'
    );
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'root-user',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'root-user',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        nickname: 'Root',
        name: 'Root',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'root',
        permissions: []
      }
    });
    applicationManagementApi.fetchSettingsApplicationManagement.mockResolvedValue(
      {
        items: [
          {
            id: 'app-workflow',
            application_type: 'workflow',
            workflow_trigger_type: 'schedule',
            name: 'Daily Report',
            description: 'Generate a report every day',
            icon: null,
            icon_type: null,
            icon_background: null,
            created_by: 'root-user',
            created_by_display_name: 'Root',
            created_at: '2026-07-12T08:00:00Z',
            updated_at: '2026-07-13T08:00:00Z',
            tags: [{ id: 'tag-report', name: '报表' }],
            publication_status: 'unpublished'
          }
        ],
        total: 21,
        page: 2,
        page_size: 20
      }
    );
    applicationsApi.fetchApplicationCatalog.mockResolvedValue({
      types: [
        { value: 'agent_flow', label: 'AgentFlow' },
        { value: 'workflow', label: 'Workflow' }
      ],
      tags: [{ id: 'tag-report', name: '报表', application_count: 1 }]
    });
  });

  test('AC-003 AC-006 restores URL filters and renders backend management fields', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    expect(await screen.findByText('Daily Report')).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: '应用管理' })
    ).not.toBeInTheDocument();
    expect(screen.getByText('Generate a report every day')).toBeInTheDocument();
    expect(screen.getAllByText('Workflow')).toHaveLength(2);
    expect(screen.getByText('定时任务')).toBeInTheDocument();
    expect(screen.getAllByText('未发布')).toHaveLength(2);
    expect(screen.getByText('Root')).toBeInTheDocument();
    expect(screen.getByText('报表')).toBeInTheDocument();
    expect(
      screen.getByRole('link', { name: 'Daily Report' })
    ).toHaveAttribute(
      'href',
      '/applications/app-workflow/orchestration'
    );

    await waitFor(() => {
      expect(
        applicationManagementApi.fetchSettingsApplicationManagement
      ).toHaveBeenCalledWith({
        page: 2,
        page_size: 20,
        filter: {
          $and: [
            { application_type: 'workflow' },
            { publication_status: 'unpublished' },
            {
              $or: [
                { name: { $includes: 'Daily' } },
                { id: { $includes: 'Daily' } }
              ]
            }
          ]
        },
        sort: 'updated_at:desc'
      });
    });
  });
});
