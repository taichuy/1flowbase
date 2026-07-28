import { afterEach, describe, expect, test, vi } from 'vitest';

vi.mock('@1flowbase/api-client', () => ({
  getConsoleApplicationOrchestration: vi.fn().mockResolvedValue({
    current_version_id: null,
    draft: null,
    messages: [
      {
        module: '@org/agent-flow/messages',
        key: 'Welcome',
        text: '欢迎'
      }
    ],
    versions: []
  }),
  getDefaultApiBaseUrl: vi.fn().mockReturnValue('http://127.0.0.1:7800'),
  restoreConsoleApplicationVersion: vi.fn().mockResolvedValue(undefined),
  saveConsoleApplicationDraft: vi.fn().mockResolvedValue(undefined)
}));

import {
  getConsoleApplicationOrchestration,
  restoreConsoleApplicationVersion,
  saveConsoleApplicationDraft,
  type SaveConsoleApplicationDraftInput
} from '@1flowbase/api-client';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import {
  fetchOrchestrationState,
  orchestrationQueryKey,
  restoreVersion,
  saveDraft
} from '../../api/orchestration';

afterEach(() => {
  vi.clearAllMocks();
});

describe('agent flow orchestration api', () => {
  test('uses a stable orchestration query key', () => {
    expect(orchestrationQueryKey('app-1')).toEqual([
      'applications',
      'app-1',
      'orchestration'
    ]);
  });

  test('passes the resolved base url when fetching orchestration state', async () => {
    const state = await fetchOrchestrationState('app-1');

    expect(getConsoleApplicationOrchestration).toHaveBeenCalledWith(
      'app-1',
      'http://127.0.0.1:7800'
    );
    expect(state.messages).toEqual([
      {
        module: '@org/agent-flow/messages',
        key: 'Welcome',
        text: '欢迎'
      }
    ]);
  });

  test('passes draft payload and csrf token unchanged when saving a draft', async () => {
    const input = {
      document: createDefaultAgentFlowDocument({ flowId: 'flow-1' }),
      change_kind: 'logical',
      summary: 'Update support flow'
    } satisfies SaveConsoleApplicationDraftInput;

    await saveDraft('app-1', input, 'csrf-123');

    expect(saveConsoleApplicationDraft).toHaveBeenCalledWith(
      'app-1',
      input,
      'csrf-123',
      'http://127.0.0.1:7800'
    );
  });

  test('passes version id and csrf token when restoring a version', async () => {
    await restoreVersion('app-1', 'version-1', 'csrf-123');

    expect(restoreConsoleApplicationVersion).toHaveBeenCalledWith(
      'app-1',
      'version-1',
      'csrf-123',
      'http://127.0.0.1:7800'
    );
  });
});
