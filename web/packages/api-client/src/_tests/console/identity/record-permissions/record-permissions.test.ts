import { beforeEach, describe, expect, test, vi } from 'vitest';
import * as transport from '../../../../transport';
import {
  fetchRecordCapabilities,
  fetchRecordPermissions,
  replaceRecordPermissions
} from '../../../../console/identity/record-permissions';

vi.mock('../../../../transport', () => ({ apiFetch: vi.fn() }));
const key = {
  plugin_code: 'ssh',
  resource_code: 'hosts',
  record_id: 'host/a?b'
};

beforeEach(() => vi.clearAllMocks());

describe('record permissions client', () => {
  test('addresses a record with encoded identifiers and no caller identity override', async () => {
    const signal = new AbortController().signal;
    await fetchRecordPermissions(key, '/host', signal);
    expect(transport.apiFetch).toHaveBeenCalledWith({
      path: '/api/console/settings/roles/record-permissions?plugin_code=ssh&resource_code=hosts&record_id=host%2Fa%3Fb',
      baseUrl: '/host',
      signal
    });
  });

  test.each(['inherit', 'restricted'] as const)(
    'keeps explicit %s mode with empty grants',
    async (mode) => {
      await replaceRecordPermissions(
        key,
        { mode, grants: [], expected_revision: 4 },
        'csrf'
      );
      expect(transport.apiFetch).toHaveBeenCalledWith(
        expect.objectContaining({
          method: 'PUT',
          csrfToken: 'csrf',
          body: { mode, grants: [], expected_revision: 4 }
        })
      );
    }
  );

  test('forwards role UUID, all action codes and the expected revision without renaming', async () => {
    const grants = [
      {
        role_id: '7cb1111b-e88f-460c-8ad5-3e16127129b6',
        actions: ['view', 'update', 'open', 'delete']
      }
    ];
    await replaceRecordPermissions(
      key,
      { mode: 'restricted', grants, expected_revision: 2 },
      'csrf'
    );
    expect(transport.apiFetch).toHaveBeenCalledWith(
      expect.objectContaining({
        body: { mode: 'restricted', grants, expected_revision: 2 }
      })
    );
  });

  test('requests a batch of server capabilities using the current session transport', async () => {
    const body = {
      plugin_code: 'ssh',
      resource_code: 'hosts',
      record_ids: ['h1', 'h2']
    };
    const signal = new AbortController().signal;
    await fetchRecordCapabilities(body, 'csrf', undefined, signal);
    expect(transport.apiFetch).toHaveBeenCalledWith({
      path: '/api/console/settings/roles/record-capabilities',
      method: 'POST',
      body,
      csrfToken: 'csrf',
      baseUrl: undefined,
      signal
    });
  });
});
