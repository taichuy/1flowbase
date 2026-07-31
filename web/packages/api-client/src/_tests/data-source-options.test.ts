import { describe, expect, test, vi } from 'vitest';

import { fetchConsoleAgentFlowDataSourceOptions } from '../console/data-source-options';
import * as transport from '../transport';

describe('agent-flow data source options client', () => {
  test('AC-001 uses the capability-filtered backend projection', async () => {
    vi.spyOn(transport, 'apiFetch').mockImplementation(
      async (input) => input as never
    );

    await expect(
      fetchConsoleAgentFlowDataSourceOptions()
    ).resolves.toMatchObject({
      path: '/api/console/data-sources/agent-flow-options'
    });
  });
});
