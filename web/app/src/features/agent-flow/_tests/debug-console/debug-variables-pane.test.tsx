import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type { AgentFlowVariableGroup } from '../../api/runtime';
import { DebugVariablesPane } from '../../components/debug-console/variables/DebugVariablesPane';

describe('DebugVariablesPane', () => {
  test('keeps duplicate variable paths selectable across groups', async () => {
    const groups: AgentFlowVariableGroup[] = [
      {
        title: 'Input Variables',
        items: [
          {
            key: 'node-start.query',
            label: 'Start/query',
            value: 'input query'
          }
        ]
      },
      {
        title: 'Node Outputs',
        items: [
          {
            key: 'node-start.query',
            label: 'Start/query',
            helperText: '用户问题',
            value: 'output query'
          }
        ]
      }
    ];
    const onSelectedChange = vi.fn();

    render(
      <DebugVariablesPane groups={groups} onSelectedChange={onSelectedChange} />
    );

    expect(screen.getByLabelText('变量值编辑框')).toHaveValue('input query');

    fireEvent.click(screen.getAllByText('Start/query')[1]);

    expect(screen.getByLabelText('变量值编辑框')).toHaveValue('output query');
    expect(screen.getByText('用户问题')).toBeInTheDocument();
    expect(onSelectedChange).toHaveBeenLastCalledWith({
      key: 'node-start.query',
      label: 'Start/query',
      value: 'output query',
      isReadOnly: undefined
    });
  });

  test('loads full runtime artifact value on explicit action', async () => {
    const groups: AgentFlowVariableGroup[] = [
      {
        title: 'Node Outputs',
        items: [
          {
            key: 'node-llm.text',
            label: 'LLM/text',
            value: {
              __runtime_debug_artifact: true,
              is_truncated: true,
              original_size_bytes: 4096,
              preview_size_bytes: 32,
              content_type: 'application/json',
              artifact_ref: 'artifact-1',
              preview: '{"text":"preview'
            },
            isTruncated: true,
            artifactRef: 'artifact-1'
          }
        ]
      }
    ];
    const onLoadFullValue = vi.fn().mockResolvedValue({ text: '完整内容' });
    const onSelectedValueChange = vi.fn();

    render(
      <DebugVariablesPane
        groups={groups}
        onLoadFullValue={onLoadFullValue}
        onSelectedValueChange={onSelectedValueChange}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '加载完整值' }));

    expect(onLoadFullValue).toHaveBeenCalledWith('artifact-1');
    await waitFor(() =>
      expect(screen.getByLabelText('变量值编辑框')).toHaveValue(
        JSON.stringify({ text: '完整内容' }, null, 2)
      )
    );
    expect(onSelectedValueChange).toHaveBeenCalledWith('node-llm.text', {
      text: '完整内容'
    });
  });
});
