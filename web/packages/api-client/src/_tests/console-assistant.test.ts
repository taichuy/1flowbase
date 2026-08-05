import { describe, expect, test, vi } from 'vitest';

import {
  getConsoleAssistantSettings,
  startConsoleAssistantRun,
  updateConsoleAssistantSettings
} from '../console-assistant';
import * as transport from '../transport';

describe('console assistant client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('AC-002 reads and writes the current assistant preference through the session API', async () => {
    await expect(getConsoleAssistantSettings()).resolves.toMatchObject({
      path: '/api/console/assistant/settings'
    });
    await expect(
      updateConsoleAssistantSettings(
        {
          application_id: 'application-1',
          mcp_instance_ids: ['catalog']
        },
        'csrf-token'
      )
    ).resolves.toMatchObject({
      path: '/api/console/assistant/settings',
      method: 'PATCH',
      csrfToken: 'csrf-token'
    });
  });

  test('AC-003 starts the assistant through its session-only route', async () => {
    await expect(
      startConsoleAssistantRun(
        {
          query: 'hello',
          history: []
        },
        'csrf-token'
      )
    ).resolves.toMatchObject({
      path: '/api/console/assistant/runs',
      method: 'POST',
      csrfToken: 'csrf-token'
    });
  });
});
