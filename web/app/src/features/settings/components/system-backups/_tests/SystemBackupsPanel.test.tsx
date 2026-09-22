import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { App } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  listSystemBackups: vi.fn(),
  getSystemBackupCatalog: vi.fn(),
  getSystemBackup: vi.fn(),
  createSystemBackup: vi.fn(),
  getSystemBackupJobStatus: vi.fn(),
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
  backup_kind: 'legacy' as const,
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
    api.getSystemBackupCatalog.mockResolvedValue({
      items: [
        {
          feature_id: 'logs',
          label_key: 'logs',
          structure_bytes: 100,
          data_bytes: 2000,
          structure_tables: ['log_settings'],
          data_tables: ['logs', 'trajectories']
        }
      ]
    });
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
        migration_head: 'migration-1'
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

  test('collects an optional backup password before verification', async () => {
    api.verifySystemBackup.mockResolvedValue({
      backup_set_id: backup.backup_set_id,
      verified: true
    });
    renderPanel();
    await screen.findByText(backup.exact_backup_name);
    await openActions();
    fireEvent.click(await screen.findByText('Verify'));
    fireEvent.change(screen.getByPlaceholderText('Optional backup password'), {
      target: { value: 'backup-password' }
    });
    fireEvent.click(screen.getByRole('button', { name: 'Verify' }));
    await waitFor(() =>
      expect(api.verifySystemBackup).toHaveBeenCalledWith(
        backup.backup_set_id,
        'csrf-token',
        undefined,
        'backup-password'
      )
    );
  });

  test('tracks a queued backup job through the server status contract', async () => {
    api.createSystemBackup.mockResolvedValue({
      backup_job_id: 'backup-job-1',
      backup_set_id: backup.backup_set_id
    });
    api.getSystemBackupJobStatus.mockResolvedValue({
      backup_job_id: 'backup-job-1',
      backup_set_id: backup.backup_set_id,
      status: 'sealing',
      failure_code: null,
      sealed_components: 3
    });

    renderPanel();
    await screen.findByText(backup.exact_backup_name);
    fireEvent.click(screen.getByRole('button', { name: /Create backup/ }));
    const createDialog = await screen.findByRole('dialog');
    fireEvent.click(
      await within(createDialog).findByRole('checkbox', { name: /STRUCTURE$/ })
    );
    fireEvent.click(
      within(createDialog).getByRole('button', { name: /Create backup/ })
    );

    await waitFor(() =>
      expect(api.getSystemBackupJobStatus).toHaveBeenCalledWith('backup-job-1')
    );
    expect(await screen.findAllByText('Backup started')).not.toHaveLength(0);
    expect(screen.getByText('backup-job-1')).toBeInTheDocument();
    expect(screen.getByText('sealing')).toBeInTheDocument();
    expect(screen.getByText('Sealed components')).toBeInTheDocument();
    expect(screen.getByText('3')).toBeInTheDocument();
  });

  test('shows the server failure code for a failed backup job', async () => {
    api.createSystemBackup.mockResolvedValue({
      backup_job_id: 'backup-job-2',
      backup_set_id: backup.backup_set_id
    });
    api.getSystemBackupJobStatus.mockResolvedValue({
      backup_job_id: 'backup-job-2',
      backup_set_id: backup.backup_set_id,
      status: 'failed',
      failure_code: 'backup_capture_failed',
      sealed_components: 1
    });

    renderPanel();
    await screen.findByText(backup.exact_backup_name);
    fireEvent.click(screen.getByRole('button', { name: /Create backup/ }));
    const createDialog = await screen.findByRole('dialog');
    fireEvent.click(
      await within(createDialog).findByRole('checkbox', { name: /STRUCTURE$/ })
    );
    fireEvent.click(
      within(createDialog).getByRole('button', { name: /Create backup/ })
    );

    expect(await screen.findByText('Backup failed')).toBeInTheDocument();
    expect(screen.getByText('backup_capture_failed')).toBeInTheDocument();
  });

  test('refreshes the backup inventory after a backup job succeeds', async () => {
    api.createSystemBackup.mockResolvedValue({
      backup_job_id: 'backup-job-3',
      backup_set_id: backup.backup_set_id
    });
    api.getSystemBackupJobStatus.mockResolvedValue({
      backup_job_id: 'backup-job-3',
      backup_set_id: backup.backup_set_id,
      status: 'succeeded',
      failure_code: null,
      sealed_components: 4
    });

    renderPanel();
    await screen.findByText(backup.exact_backup_name);
    fireEvent.click(screen.getByRole('button', { name: /Create backup/ }));
    const createDialog = await screen.findByRole('dialog');
    fireEvent.click(
      await within(createDialog).findByRole('checkbox', { name: /STRUCTURE$/ })
    );
    fireEvent.click(
      within(createDialog).getByRole('button', { name: /Create backup/ })
    );

    expect(await screen.findByText('Backup completed')).toBeInTheDocument();
    await waitFor(() => expect(api.listSystemBackups).toHaveBeenCalledTimes(2));
  });

  test('requires a selection, keeps STRUCTURE and DATA independent, and defaults file bytes off', async () => {
    api.createSystemBackup.mockResolvedValue({
      backup_job_id: 'selected-job',
      backup_set_id: backup.backup_set_id
    });
    api.getSystemBackupJobStatus.mockResolvedValue({
      status: 'queued',
      sealed_components: 0
    });
    renderPanel();
    fireEvent.click(
      await screen.findByRole('button', { name: /Create backup/ })
    );
    const dialog = await screen.findByRole('dialog');
    const create = within(dialog).getByRole('button', {
      name: /Create backup/
    });
    expect(create).toBeDisabled();
    const structure = await within(dialog).findByRole('checkbox', {
      name: /STRUCTURE$/
    });
    const data = within(dialog).getByRole('checkbox', { name: /DATA$/ });
    const files = within(dialog).getByRole('checkbox', {
      name: 'Include file bytes (optional)'
    });
    expect(structure).not.toBeChecked();
    expect(data).not.toBeChecked();
    expect(files).not.toBeChecked();
    expect(
      within(dialog).getByText(/including logs and execution trajectories/)
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(/Plugin packages are not included/)
    ).toBeInTheDocument();
    fireEvent.click(data);
    expect(structure).not.toBeChecked();
    expect(
      within(dialog).getByText(
        'Selected records: approximately 2.0 KB before compression'
      )
    ).toBeInTheDocument();
    fireEvent.click(create);
    await waitFor(() =>
      expect(api.createSystemBackup).toHaveBeenCalledWith(
        'csrf-token',
        undefined,
        {
          backup_password: undefined,
          selection: {
            features: [{ feature_id: 'logs', structure: false, data: true }],
            include_file_bytes: false
          }
        }
      )
    );
  });

  test('includes explicitly selected file bytes while retaining the backup password', async () => {
    api.createSystemBackup.mockResolvedValue({
      backup_job_id: 'files-job',
      backup_set_id: backup.backup_set_id
    });
    api.getSystemBackupJobStatus.mockResolvedValue({
      status: 'queued',
      sealed_components: 0
    });
    renderPanel();
    fireEvent.click(
      await screen.findByRole('button', { name: /Create backup/ })
    );
    const dialog = await screen.findByRole('dialog');
    fireEvent.click(
      await within(dialog).findByRole('checkbox', { name: /STRUCTURE$/ })
    );
    fireEvent.click(within(dialog).getByRole('checkbox', { name: /DATA$/ }));
    fireEvent.click(
      within(dialog).getByRole('checkbox', {
        name: 'Include file bytes (optional)'
      })
    );
    fireEvent.change(
      within(dialog).getByPlaceholderText('Optional backup password'),
      { target: { value: 'archive-secret' } }
    );
    fireEvent.click(
      within(dialog).getByRole('button', { name: /Create backup/ })
    );
    await waitFor(() =>
      expect(api.createSystemBackup).toHaveBeenCalledWith(
        'csrf-token',
        undefined,
        {
          backup_password: 'archive-secret',
          selection: {
            features: [{ feature_id: 'logs', structure: true, data: true }],
            include_file_bytes: true
          }
        }
      )
    );
  });

  test('requires explicit missing-plugin confirmation and completes selective restoration without offline polling', async () => {
    const legacy = await api.preflightSystemRecovery();
    api.preflightSystemRecovery.mockResolvedValue({
      ...legacy,
      impact: { ...legacy.impact, database_replaced: false },
      selective: {
        failures: [],
        missing_plugins: ['example.plugin'],
        table_count: 2,
        row_count: 37
      }
    });
    api.reauthenticateSystemRecovery.mockResolvedValue({
      challenge_token: 'challenge'
    });
    api.createSystemRecoveryIntent.mockResolvedValue({
      status: 'succeeded',
      restart_required: true,
      recovery_job_id: 'selective-job'
    });
    renderPanel();
    await openActions();
    fireEvent.click(await screen.findByText('Restore'));
    expect(await screen.findByText('example.plugin')).toBeInTheDocument();
    expect(
      screen.getByText('2 tables and 37 records will be imported.')
    ).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText('Current password'), {
      target: { value: 'password' }
    });
    fireEvent.change(screen.getByPlaceholderText(backup.exact_backup_name), {
      target: { value: backup.exact_backup_name }
    });
    const confirm = screen.getByRole('button', {
      name: 'Confirm and import records'
    });
    expect(confirm).toBeDisabled();
    expect(api.reauthenticateSystemRecovery).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole('checkbox', {
        name: /I understand these plugins are missing/
      })
    );
    fireEvent.click(confirm);
    expect(
      await screen.findByText('Records restored successfully')
    ).toBeInTheDocument();
    expect(api.createSystemRecoveryIntent).toHaveBeenCalledWith(
      backup.backup_set_id,
      expect.objectContaining({
        confirm_missing_plugins: true,
        challenge_token: 'challenge'
      }),
      'csrf-token'
    );
    expect(
      screen.getByText(
        'Restart the server after recovery to load restored settings and plugin registrations.'
      )
    ).toBeInTheDocument();
    expect(api.getSystemRecoveryStatus).not.toHaveBeenCalled();
  });

  test('blocks selective schema failures even with password and exact name', async () => {
    const legacy = await api.preflightSystemRecovery();
    api.preflightSystemRecovery.mockResolvedValue({
      ...legacy,
      selective: {
        failures: ['Required table is missing'],
        missing_plugins: [],
        table_count: 1,
        row_count: 2
      }
    });
    renderPanel();
    await openActions();
    fireEvent.click(await screen.findByText('Restore'));
    expect(
      await screen.findByText('Required table is missing')
    ).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText('Current password'), {
      target: { value: 'password' }
    });
    fireEvent.change(screen.getByPlaceholderText(backup.exact_backup_name), {
      target: { value: backup.exact_backup_name }
    });
    expect(
      screen.getByRole('button', { name: 'Confirm and import records' })
    ).toBeDisabled();
    expect(api.reauthenticateSystemRecovery).not.toHaveBeenCalled();
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
