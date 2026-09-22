import { beforeEach, describe, expect, test, vi } from 'vitest';

import * as transport from '../../transport';
import {
  createSystemBackup,
  getSystemBackupCatalog,
  createSystemRecoveryIntent,
  getSystemBackupDownloadUrl,
  getSystemBackupJobStatus
} from '../system-backups';

describe('system backup transport contract', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(transport, 'apiFetch').mockImplementation(
      async (input) => input as never
    );
  });

  test('returns a same-origin authenticated download URL without fetching or buffering a Blob', () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    const blobSpy = vi.spyOn(Response.prototype, 'blob');

    expect(
      getSystemBackupDownloadUrl(
        '0198f8e1-21e0-7000-8000-000000000001',
        'https://console.example.test'
      )
    ).toBe(
      'https://console.example.test/api/console/settings/system-backups/0198f8e1-21e0-7000-8000-000000000001/download'
    );
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(blobSpy).not.toHaveBeenCalled();
  });

  test('loads the settings catalog and forwards explicit missing-plugin confirmation', async () => {
    await expect(getSystemBackupCatalog()).resolves.toMatchObject({
      path: '/api/console/settings/system-backups/catalog'
    });
    const request = {
      challenge_token: 'challenge',
      exact_backup_name: 'backup',
      plan_digest: 'digest',
      confirm_missing_plugins: true
    };
    await expect(
      createSystemRecoveryIntent('backup', request, 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/system-backups/backup/recovery/intents',
      method: 'POST',
      body: request,
      csrfToken: 'csrf'
    });
  });

  test('uses the queued backup response and job status endpoint', async () => {
    await expect(
      createSystemBackup('csrf-token', undefined, {
        backup_password: 'backup-password',
        selection: {
          features: [{ feature_id: 'logs', structure: false, data: true }],
          include_file_bytes: false
        }
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/system-backups',
      method: 'POST',
      csrfToken: 'csrf-token',
      body: {
        backup_password: 'backup-password',
        selection: {
          features: [{ feature_id: 'logs', structure: false, data: true }],
          include_file_bytes: false
        }
      }
    });

    await expect(
      getSystemBackupJobStatus('backup-job-1')
    ).resolves.toMatchObject({
      path: '/api/console/settings/system-backups/jobs/status/backup-job-1'
    });
  });
});
