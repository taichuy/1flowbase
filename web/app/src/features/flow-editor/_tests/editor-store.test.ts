import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { describe, expect, test } from 'vitest';

import { createFlowEditorStore } from '../store';

describe('flow editor store', () => {
  test('AC-002 owns only application-neutral editor state', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const store = createFlowEditorStore({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-07-10T10:00:00Z',
        document
      },
      autosave_interval_seconds: 30,
      user_protection_limit: 10,
      versions: []
    });
    const state = store.getState() as unknown as Record<string, unknown>;

    expect(state.workingDocument).toBe(document);
    expect(state.issuesOpen).toBe(false);
    expect(state.historyOpen).toBe(false);
    expect(state).not.toHaveProperty('debugConsoleOpen');
    expect(state).not.toHaveProperty('debugConsoleWidth');
    expect(state).not.toHaveProperty('publishConfigOpen');
    expect(state).not.toHaveProperty('nodeDetailTab');
  });
});
