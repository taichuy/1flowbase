import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { StartModelListField } from '../../components/detail/fields/StartModelListField';

describe('StartModelListField', () => {
  test('shows only model id and context in the list', () => {
    const onChange = vi.fn();

    const { container } = render(
      <StartModelListField
        title="模型列表"
        value={[
          {
            id: ' qwen3.6-35b-a3b ',
            name: ' Qwen 3.6 35B ',
            context_window: 128000,
            max_context_window: 200000,
            max_output_tokens: 8192,
            auto_compact_token_limit: 110000,
            capabilities: {
              tool_call: true,
              multimodal: true,
              structured_output: true
            },
            reasoning: {
              default_effort: 'medium',
              supported_efforts: ['low', 'medium']
            }
          },
          ' deepseek-v4-flash '
        ]}
        onChange={onChange}
      />
    );

    expect(screen.getByText('qwen3.6-35b-a3b')).toBeInTheDocument();
    expect(screen.getByText('128K')).toBeInTheDocument();
    expect(screen.getByText('deepseek-v4-flash')).toBeInTheDocument();
    expect(
      container.querySelector('.agent-flow-node-detail__list-item-icon')
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText('支持的推理强度 1')).not.toBeInTheDocument();
    expect(
      screen.queryByPlaceholderText('display name')
    ).not.toBeInTheDocument();
  });

  test('adds the default flowbase model with real defaults after confirmation', () => {
    const onChange = vi.fn();

    render(
      <StartModelListField title="模型列表" value={[]} onChange={onChange} />
    );

    const header = screen.getByTestId('agent-flow-collection-field-header');

    expect(
      within(header).getByRole('heading', { name: '模型列表' })
    ).toBeInTheDocument();
    expect(
      within(header).getByRole('button', { name: '新增模型' })
    ).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText('新增模型'));
    expect(screen.getByLabelText('模型 ID 输入')).toHaveValue('flowbase');
    expect(screen.getByLabelText('模型显示名输入')).toHaveValue('flowbase');
    expect(screen.getByLabelText('模型上下文窗口')).toHaveValue('128');
    expect(screen.getByLabelText('最大上下文窗口输入')).toHaveValue('128');
    expect(screen.getByLabelText('最大输出 Token 输入')).toHaveValue('8');
    expect(screen.getByLabelText('自动压缩阈值百分比输入')).toHaveValue('85');
    expect(
      screen.queryByLabelText('External reasoning override switch')
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText('支持的推理强度 1')).toHaveValue('minimal');
    expect(screen.getByLabelText('支持的推理强度 2')).toHaveValue('low');
    expect(screen.getByLabelText('支持的推理强度 3')).toHaveValue('medium');
    expect(screen.getByLabelText('支持的推理强度 4')).toHaveValue('high');
    expect(screen.getByLabelText('支持的推理强度 5')).toHaveValue('xhigh');
    fireEvent.click(screen.getByLabelText('保存模型'));

    expect(onChange).toHaveBeenLastCalledWith([
      {
        id: 'flowbase',
        name: 'flowbase',
        context_window: 128000,
        max_context_window: 128000,
        max_output_tokens: 8000,
        auto_compact_token_limit: 108800,
        capabilities: {
          reasoning: true,
          tool_call: true,
          multimodal: true,
          structured_output: true
        },
        reasoning: {
          default_effort: 'medium',
          supported_efforts: ['minimal', 'low', 'medium', 'high', 'xhigh']
        }
      }
    ]);
  });

  test('links derived token defaults, preserves custom values, and rejects an oversized context', () => {
    render(<StartModelListField value={[]} onChange={vi.fn()} />);

    fireEvent.click(screen.getByLabelText('新增模型'));

    const contextInput = screen.getByLabelText('模型上下文窗口');
    const maxContextInput = screen.getByLabelText('最大上下文窗口输入');
    const outputInput = screen.getByLabelText('最大输出 Token 输入');
    const compactInput = screen.getByLabelText('自动压缩阈值百分比输入');
    const saveButton = screen.getByLabelText('保存模型');

    expect(
      maxContextInput.compareDocumentPosition(contextInput) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();

    fireEvent.change(maxContextInput, { target: { value: '353' } });
    expect(contextInput).toHaveValue('353');
    expect(outputInput).toHaveValue('32');
    expect(compactInput).toHaveValue('85');

    fireEvent.change(contextInput, { target: { value: '300' } });
    fireEvent.change(outputInput, { target: { value: '64' } });
    fireEvent.change(compactInput, { target: { value: '80' } });
    fireEvent.change(maxContextInput, { target: { value: '400' } });
    expect(contextInput).toHaveValue('300');
    expect(outputInput).toHaveValue('64');
    expect(compactInput).toHaveValue('80');

    fireEvent.change(outputInput, { target: { value: '32' } });
    fireEvent.change(compactInput, { target: { value: '85' } });
    fireEvent.change(contextInput, { target: { value: '353' } });
    fireEvent.change(maxContextInput, { target: { value: '500' } });
    expect(contextInput).toHaveValue('353');
    expect(outputInput).toHaveValue('32');
    expect(compactInput).toHaveValue('85');

    fireEvent.change(maxContextInput, { target: { value: '300' } });
    expect(saveButton).toBeDisabled();
    expect(
      screen.getByText('上下文窗口不能超过最大上下文窗口')
    ).toBeInTheDocument();
  });

  test('keeps the display name linked until it is customized', () => {
    render(
      <StartModelListField title="模型列表" value={[]} onChange={vi.fn()} />
    );

    fireEvent.click(screen.getByLabelText('新增模型'));

    const modelIdInput = screen.getByLabelText('模型 ID 输入');
    const modelNameInput = screen.getByLabelText('模型显示名输入');

    fireEvent.change(modelIdInput, {
      target: { value: 'gpt-5.5' }
    });
    expect(modelNameInput).toHaveValue('gpt-5.5');

    fireEvent.change(modelNameInput, {
      target: { value: 'GPT 5.5' }
    });
    fireEvent.change(modelIdInput, {
      target: { value: 'gpt-5.5-pro' }
    });
    expect(modelNameInput).toHaveValue('GPT 5.5');

    fireEvent.change(modelNameInput, {
      target: { value: 'gpt-5.5-pro' }
    });
    fireEvent.change(modelIdInput, {
      target: { value: 'gpt-5.5-latest' }
    });
    expect(modelNameInput).toHaveValue('gpt-5.5-latest');
  });

  test('saves optional model settings when configured in the form', () => {
    const onChange = vi.fn();

    render(
      <StartModelListField title="模型列表" value={[]} onChange={onChange} />
    );

    fireEvent.click(screen.getByLabelText('新增模型'));
    fireEvent.change(screen.getByLabelText('模型 ID 输入'), {
      target: { value: 'gpt-5.5' }
    });
    fireEvent.change(screen.getByLabelText('模型显示名输入'), {
      target: { value: 'GPT 5.5' }
    });
    expect(
      screen.getAllByLabelText('模型上下文窗口单位').length
    ).toBeGreaterThan(0);
    fireEvent.change(screen.getByLabelText('模型上下文窗口'), {
      target: { value: '256' }
    });
    fireEvent.change(screen.getByLabelText('最大上下文窗口输入'), {
      target: { value: '300' }
    });
    fireEvent.change(screen.getByLabelText('最大输出 Token 输入'), {
      target: { value: '64' }
    });
    fireEvent.change(screen.getByLabelText('自动压缩阈值百分比输入'), {
      target: { value: '85' }
    });
    fireEvent.click(screen.getByLabelText('删除支持的推理强度 1'));
    fireEvent.click(screen.getByLabelText('删除支持的推理强度 4'));
    expect(screen.getByLabelText('支持的推理强度 1')).toHaveValue('low');
    expect(screen.getByLabelText('支持的推理强度 2')).toHaveValue('medium');
    expect(screen.getByLabelText('支持的推理强度 3')).toHaveValue('high');
    fireEvent.click(screen.getByLabelText('新增支持的推理强度'));
    fireEvent.change(screen.getByLabelText('支持的推理强度 4'), {
      target: { value: 'ultra' }
    });
    expect(screen.getAllByLabelText('默认推理强度输入').length).toBeGreaterThan(
      0
    );
    expect(screen.getByLabelText('工具调用能力')).toBeChecked();
    expect(screen.getByLabelText('多模态能力')).toBeChecked();
    expect(screen.getByLabelText('结构化输出能力')).toBeChecked();
    fireEvent.click(screen.getByLabelText('保存模型'));

    expect(onChange).toHaveBeenLastCalledWith([
      {
        id: 'gpt-5.5',
        name: 'GPT 5.5',
        context_window: 256000,
        max_context_window: 300000,
        max_output_tokens: 64000,
        auto_compact_token_limit: 217600,
        capabilities: {
          reasoning: true,
          tool_call: true,
          multimodal: true,
          structured_output: true
        },
        reasoning: {
          default_effort: 'medium',
          supported_efforts: ['low', 'medium', 'high', 'ultra']
        }
      }
    ]);
  });

  test('keeps focus when editing a newly added reasoning effort', () => {
    const onChange = vi.fn();

    render(
      <StartModelListField title="模型列表" value={[]} onChange={onChange} />
    );

    fireEvent.click(screen.getByLabelText('新增模型'));
    fireEvent.click(screen.getByLabelText('新增支持的推理强度'));

    const effortInput = screen.getByLabelText('支持的推理强度 6');
    act(() => {
      effortInput.focus();
    });

    fireEvent.change(effortInput, {
      target: { value: 'u' }
    });

    expect(screen.getByLabelText('支持的推理强度 6')).toHaveFocus();
  });

  test('does not wrap floating model form controls in native label rows', () => {
    render(
      <StartModelListField title="模型列表" value={[]} onChange={vi.fn()} />
    );

    fireEvent.click(screen.getByLabelText('新增模型'));

    expect(
      screen.queryByLabelText('Input placeholder')
    ).not.toBeInTheDocument();
  });
});
