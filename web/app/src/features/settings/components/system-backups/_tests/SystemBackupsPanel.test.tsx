import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { App } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  listSystemBackups: vi.fn(),
  getSystemBackup: vi.fn(),
  createSystemBackup: vi.fn(),
  importSystemBackup: vi.fn(),
  verifySystemBackup: vi.fn(),
  deleteSystemBackup: vi.fn(),
  getSystemBackupDownloadUrl: vi.fn(),
  preflightSystemRecovery: vi.fn(),
  reauthenticateSystemRecovery: vi.fn(),
  createSystemRecoveryIntent: vi.fn(),
  getSystemRecoveryStatus: vi.fn()
}));

vi.mock('@1flowbase/api-client', () => api);

import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { useAuthStore } from '../../../../../state/auth-store';
import { SystemBackupsPanel } from '../SystemBackupsPanel';

const backup = {
  backup_set_id: '0198f8e1-21e0-7000-8000-000000000001',
  exact_backup_name: '0198f8e1-21e0-7000-8000-000000000001',
  created_at: '2026-08-12T08:00:00Z',
  availability: 'ready' as const,
  total_size_bytes: 1024,
  envelope_digest: 'a'.repeat(64)
};

function renderPanel() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  return render(
    <App>
      <QueryClientProvider client={queryClient}>
        <SystemBackupsPanel />
      </QueryClientProvider>
    </App>
  );
}

async function openActions() {
  fireEvent.click(await screen.findByRole('button', { name: 'Actions' }));
}

describe('SystemBackupsPanel', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await appI18n.changeLanguage('en_US');
    useAuthStore.setState({ csrfToken: 'csrf-token' });
    api.listSystemBackups.mockResolvedValue({ items: [backup] });
    api.getSystemBackup.mockResolvedValue({
      backup_set_id: backup.backup_set_id,
      exact_backup_name: backup.exact_backup_name,
      created_at: backup.created_at,
      content: {
        component_count: 2,
        postgresql_count: 1,
        business_object_count: 1,
        extension_artifact_count: 0,
        mcp_artifact_count: 0,
        embedded_component_count: 2,
        identity_only_component_count: 0,
        total_size_bytes: backup.total_size_bytes,
        excluded_domains: ['ephemeral_state']
      },
      components: [
        {
          component_id: 'postgresql/main',
          kind: 'postgres_sql',
          source_identity: 'postgresql/main',
          content_type: 'application/vnd.postgresql.custom',
          size_bytes: 512,
          content_digest: 'c'.repeat(64),
          disposition: 'embedded',
          rebuildability: 'not_applicable',
          restore_target: { target_kind: 'postgre_sql' }
        }
      ],
      compatibility: {
        compatible: true,
        failures: [],
        format_version: 1,
        application_build: 'build-1',
        migration_head: 'migration-1',
        master_key_fingerprint: 'd'.repeat(64)
      },
      verification: { verified: true, checked_at: backup.created_at },
      creation_journal: [
        {
          sequence: 0,
          occurred_at: backup.created_at,
          state: 'available',
          component_id: null,
          failure_code: null
        }
      ],
      recovery_history: []
    });
    api.getSystemBackupDownloadUrl.mockReturnValue(
      `/api/console/settings/system-backups/${backup.backup_set_id}/download`
    );
    api.preflightSystemRecovery.mockResolvedValue({
      backup_set_id: backup.backup_set_id,
      plan_digest: 'b'.repeat(64),
      compatible: true,
      required_space_bytes: 4096,
      available_space_bytes: 8192,
      impact: {
        database_replaced: true,
        business_object_count: 2,
        extension_artifact_count: 1,
        mcp_artifact_count: 1,
        active_work: []
      },
      failures: []
    });
  });

  test('has no batch selection and never renders raw sealed manifest JSON', async () => {
    renderPanel();
    expect(
      await screen.findByText(backup.exact_backup_name)
    ).toBeInTheDocument();
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
    expect(screen.queryByText('Batch actions')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText(backup.exact_backup_name));
    expect(await screen.findByText('Backup details')).toBeInTheDocument();
    await waitFor(() =>
      expect(api.getSystemBackup).toHaveBeenCalledWith(backup.backup_set_id)
    );
    expect(screen.queryByText('must-not-render')).not.toBeInTheDocument();
    expect(screen.queryByText('internal_secret')).not.toBeInTheDocument();
    expect(await screen.findByText('Component inventory')).toBeInTheDocument();
    expect(await screen.findByText('Integrity verified')).toBeInTheDocument();
  });

  test('uses direct authenticated download without Blob buffering', async () => {
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => {});
    renderPanel();
    await screen.findByText(backup.exact_backup_name);
    await openActions();
    fireEvent.click(await screen.findByText('Download'));

    expect(api.getSystemBackupDownloadUrl).toHaveBeenCalledWith(
      backup.backup_set_id
    );
    expect(click).toHaveBeenCalledOnce();
  });

  test('keeps restore dangerous and projects server preflight and journal status', async () => {
    api.reauthenticateSystemRecovery.mockResolvedValue({
      challenge_token: 'challenge',
      expires_at: '2026-08-12T08:05:00Z'
    });
    api.createSystemRecoveryIntent.mockResolvedValue({
      intent_id: 'intent',
      recovery_job_id: 'job-1',
      backup_set_id: backup.backup_set_id,
      status: 'preparing',
      expires_at: '2026-08-12T08:02:00Z'
    });
    api.getSystemRecoveryStatus.mockResolvedValue({
      phase: 'active',
      recovery_job_id: 'job-1',
      active_write_count: 0,
      started_at: '2026-08-12T08:00:00Z',
      target_backup_set_id: backup.backup_set_id,
      safety_backup_set_id: 'safe-1',
      plan_digest: 'b'.repeat(64),
      journal_state: 'restoring',
      journal_events: []
    });

    renderPanel();
    await screen.findByText(backup.exact_backup_name);
    await openActions();
    fireEvent.click(await screen.findByText('Restore'));
    expect(await screen.findByText('Preflight passed')).toBeInTheDocument();
    expect(screen.getByText('Replace database')).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText('Current password'), {
      target: { value: 'change-me' }
    });
    fireEvent.change(screen.getByPlaceholderText(backup.exact_backup_name), {
      target: { value: backup.exact_backup_name }
    });
    const confirm = screen.getByRole('button', {
      name: 'Confirm and prepare recovery'
    });
    expect(confirm).toHaveClass('ant-btn-dangerous');
    fireEvent.click(confirm);

    await waitFor(() =>
      expect(api.getSystemRecoveryStatus).toHaveBeenCalledWith('job-1')
    );
    expect(await screen.findByText('restoring')).toBeInTheDocument();
    expect(screen.getByText('safe-1')).toBeInTheDocument();
  });
});
