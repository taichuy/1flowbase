import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { ReadOnlyConversationInput } from '../../../components/debug-console/conversation/read-only/ReadOnlyConversationInput';

describe('ReadOnlyConversationInput', () => {
  test('uses the shared chat composer with disabled input and send action', () => {
    render(<ReadOnlyConversationInput model="gpt-6-luna" reasoning_effort="max" />);

    const input = screen.getByRole('textbox', { name: '只读消息输入框' });
    expect(input.closest('.ant-sender')).toBeInTheDocument();
    expect(input.closest('.agent-flow-editor__debug-composer')).toBeInTheDocument();
    expect(input).toBeDisabled();
    expect(input).toHaveValue('');
    expect(input).toHaveAttribute('placeholder', '和 Bot 聊天');

    const sendButton = screen.getByRole('button', { name: '发送调试消息' });
    expect(sendButton).toBeDisabled();
    fireEvent.keyDown(input, { key: 'Enter', code: 'Enter' });
    fireEvent.click(sendButton);
    expect(input).toHaveValue('');
    expect(screen.queryByRole('button', { name: '终止调试运行' })).not.toBeInTheDocument();

    const footer = input.closest('.ant-sender')?.querySelector('.ant-sender-footer');
    expect(footer).toContainElement(screen.getByText('gpt-6-luna'));
    expect(footer).toContainElement(screen.getByText('max'));
  });

  test('keeps missing model metadata non-interactive', () => {
    render(<ReadOnlyConversationInput />);

    expect(screen.getAllByText('—')).toHaveLength(2);
    expect(screen.getByRole('textbox')).toBeDisabled();
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
  });
});
