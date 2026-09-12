import { render, screen } from '@testing-library/react';
import type { TFunction } from 'i18next';
import { describe, expect, test } from 'vitest';

import type { ApplicationRunSummary } from '../../api/runtime';
import { getApplicationRunsTableColumns } from '../../components/logs/application-runs-table-columns';

// #2034 AC-008: the original title column reports compaction calls separately
// from the user-facing invocation count, using backend field names verbatim.
const t = ((key: string, options?: { count?: number }) =>
  options?.count === undefined
    ? key
    : `${key}:${options.count}`) as unknown as TFunction<'applications'>;

function summary(
  overrides: Partial<ApplicationRunSummary>
): ApplicationRunSummary {
  return {
    log_conversation_id: 'conversation-1',
    log_task_run_id: 'run-1',
    parent_run_id: null,
    caused_by_run_id: null,
    invocation_count: 1,
    compaction_count: 0,
    call_kind: 'generate',
    id: 'run-1',
    application_id: 'app-1',
    scope_id: 'workspace-1',
    run_mode: 'published_api_run',
    execution_stage: 'published',
    invocation_source: 'agent_flow_api',
    principal: { kind: 'application_api_key', id: 'key-1', display_name: null },
    status: 'succeeded',
    target_node_id: null,
    title: 'refactor login',
    total_tokens: 7600,
    input_tokens: null,
    output_tokens: null,
    count_tokens_input_tokens: null,
    input_cache_hit_tokens: null,
    input_cache_hit_rate: null,
    unique_node_count: 1,
    tool_callback_count: 0,
    started_at: '2026-09-12T00:00:00Z',
    finished_at: '2026-09-12T00:00:01Z',
    created_at: '2026-09-12T00:00:00Z',
    updated_at: '2026-09-12T00:00:01Z',
    ...overrides
  } as ApplicationRunSummary;
}

function renderTitle(record: ApplicationRunSummary) {
  const column = getApplicationRunsTableColumns(t).find(
    (c) => c.key === 'title'
  );
  render(<>{column?.render?.(record.title, record, 0)}</>);
}

describe('task call kind tag', () => {
  test('shows generate invocations and compactions as separate tags', () => {
    renderTitle(summary({ invocation_count: 3, compaction_count: 1 }));
    expect(
      screen.getByText('auto.task_invocation_count:3')
    ).toBeInTheDocument();
    expect(
      screen.getByText('auto.task_compaction_count:1')
    ).toBeInTheDocument();
  });

  test('omits both tags for a single plain call', () => {
    renderTitle(summary({}));
    expect(screen.queryByText(/auto\.task_invocation_count/)).toBeNull();
    expect(screen.queryByText(/auto\.task_compaction_count/)).toBeNull();
  });
});
