import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, test, vi } from 'vitest';

import type { SchemaAdapter } from '../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import { LlmMcpInstancesField } from '../../components/detail/fields/LlmMcpInstancesField';

vi.mock('../../api/mcp-instance-options', () => ({
  agentFlowMcpInstanceOptionsQueryKey: ['agent-flow', 'mcp-instance-options'],
  fetchAgentFlowMcpInstanceOptions: vi.fn(async () => [
    {
      value: 'workspace-ops',
      label: 'Workspace Ops',
      registrationPrefix: 'workspace_ops'
    },
    {
      value: 'knowledge',
      label: 'Knowledge',
      registrationPrefix: 'knowledge'
    }
  ])
}));

function McpInstancesFieldHarness({
  initialValue
}: {
  initialValue: string[];
}) {
  const [value, setValue] = useState(initialValue);
  const [queryClient] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } })
  );
  const adapter: SchemaAdapter = {
    getValue: (path) => (path === 'config.mcp_instance_ids' ? value : null),
    setValue: (path, nextValue) => {
      if (path === 'config.mcp_instance_ids' && Array.isArray(nextValue)) {
        setValue(
          nextValue.filter((item): item is string => typeof item === 'string')
        );
      }
    },
    getDerived: () => null,
    dispatch: vi.fn()
  };

  return (
    <QueryClientProvider client={queryClient}>
      <LlmMcpInstancesField
        adapter={adapter}
        block={{
          kind: 'field',
          label: '挂载 MCP',
          path: 'config.mcp_instance_ids',
          renderer: 'llm_mcp_instances'
        }}
      />
      <output data-testid="mcp-instance-ids-value">
        {JSON.stringify(value)}
      </output>
    </QueryClientProvider>
  );
}

describe('LlmMcpInstancesField', () => {
  test('AC-011 AC-012 AC-014 preserves duplicate occurrences and removes only the selected row', async () => {
    render(
      <McpInstancesFieldHarness
        initialValue={['workspace-ops', 'workspace-ops', 'missing-instance']}
      />
    );

    expect(
      screen.getByTestId('agent-flow-mcp-instances-toolbar')
    ).toHaveTextContent('挂载 MCP');

    await waitFor(() =>
      expect(
        screen.getAllByTestId(/agent-flow-mcp-instance-occurrence-/)
      ).toHaveLength(3)
    );
    expect(screen.getByText('missing-instance')).toBeInTheDocument();
    expect(await screen.findByText('不可用')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '添加 MCP 实例' }));
    fireEvent.click(
      await screen.findByRole('menuitem', { name: 'Workspace Ops' })
    );

    await waitFor(() =>
      expect(
        screen.getAllByTestId(/agent-flow-mcp-instance-occurrence-/)
      ).toHaveLength(4)
    );
    expect(screen.getByTestId('mcp-instance-ids-value')).toHaveTextContent(
      '["workspace-ops","workspace-ops","missing-instance","workspace-ops"]'
    );

    const firstOccurrence = screen.getByTestId(
      'agent-flow-mcp-instance-occurrence-0'
    );
    fireEvent.click(
      within(firstOccurrence).getByRole('button', {
        name: '删除 Workspace Ops'
      })
    );

    await waitFor(() =>
      expect(screen.getByTestId('mcp-instance-ids-value')).toHaveTextContent(
        '["workspace-ops","missing-instance","workspace-ops"]'
      )
    );
    expect(
      screen.getAllByTestId(/agent-flow-mcp-instance-occurrence-/)
    ).toHaveLength(3);
  });

  test('AC-011 AC-013 renders a collection empty state without an enable switch', async () => {
    render(<McpInstancesFieldHarness initialValue={[]} />);

    expect(await screen.findByText('暂无 MCP 实例挂载')).toBeInTheDocument();
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });
});
