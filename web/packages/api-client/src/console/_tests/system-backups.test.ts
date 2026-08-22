import { beforeEach, describe, expect, test, vi } from 'vitest';

import * as transport from '../../transport';
import {
  createSystemBackup,
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

  test('uses the queued backup response and job status endpoint', async () => {
    await expect(
      createSystemBackup('csrf-token', undefined, {
        backup_password: 'backup-password'
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/system-backups',
      method: 'POST',
      csrfToken: 'csrf-token',
      body: { backup_password: 'backup-password' }
    });

    await expect(getSystemBackupJobStatus('backup-job-1')).resolves.toMatchObject({
      path: '/api/console/settings/system-backups/jobs/backup-job-1'
    });
  });
});
