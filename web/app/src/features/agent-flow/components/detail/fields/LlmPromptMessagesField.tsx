import { DeleteOutlined, DownOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Dropdown, Typography } from 'antd';
import { useState } from 'react';

import type {
  LlmPromptMessage,
  LlmPromptMessageRole
} from '@1flowbase/flow-schema';

import { TemplatedTextField } from '../../bindings/TemplatedTextField';
import {
  createPromptMessage,
  LLM_PROMPT_MESSAGE_ROLES
} from '../../../lib/llm-prompt-messages';
import type { FlowSelectorOption } from '../../../lib/selector-options';
import { i18nText } from '../../../../../shared/i18n/text';

const DYNAMIC_PROMPT_MESSAGE_ROLES = LLM_PROMPT_MESSAGE_ROLES.filter(
  (role) => role !== 'system'
);

interface LlmPromptMessagesFieldProps {
  value: LlmPromptMessage[];
  options: FlowSelectorOption[];
  onChange: (value: LlmPromptMessage[]) => void;
}

function moveItem<T>(items: T[], from: number, to: number) {
  if (
    from === to ||
    from < 0 ||
    to < 0 ||
    from >= items.length ||
    to >= items.length
  ) {
    return items;
  }

  const nextItems = [...items];
  const [item] = nextItems.splice(from, 1);

  if (!item) {
    return items;
  }

  nextItems.splice(to, 0, item);
  return nextItems;
}

function updateAt(
  messages: LlmPromptMessage[],
  index: number,
  patch: Partial<LlmPromptMessage>
) {
  return messages.map((message, messageIndex) =>
    messageIndex === index ? { ...message, ...patch } : message
  );
}

function normalizeMessageGroups(messages: LlmPromptMessage[]) {
  const systemMessage =
    messages[0]?.role === 'system'
      ? messages[0]
      : createPromptMessage('system', 0);
  const dynamicMessages =
    messages[0]?.role === 'system'
      ? messages.slice(1)
      : messages.filter((message) => message.role !== 'system');

  return {
    systemMessage,
    dynamicMessages,
    orderedMessages: [systemMessage, ...dynamicMessages]
  };
}

interface PromptMessageRoleSelectProps {
  ariaLabel: string;
  value: LlmPromptMessageRole;
  onChange: (role: LlmPromptMessageRole) => void;
}

function PromptMessageRoleSelect({
  ariaLabel,
  value,
  onChange
}: PromptMessageRoleSelectProps) {
  const roleLabel = value.toUpperCase();

  return (
    <Dropdown
      menu={{
        className: 'agent-flow-llm-prompt-messages__role-menu',
        items: DYNAMIC_PROMPT_MESSAGE_ROLES.map((role) => ({
          key: role,
          label: role.toUpperCase()
        })),
        onClick: ({ key }) => onChange(key as LlmPromptMessageRole),
        selectedKeys: [value]
      }}
      classNames={{ root: 'agent-flow-llm-prompt-messages__role-dropdown' }}
      placement="bottomLeft"
      trigger={['click']}
    >
      <button
        aria-label={ariaLabel}
        className="agent-flow-llm-prompt-messages__role-trigger"
        type="button"
        onClick={(event) => event.preventDefault()}
      >
        <span>{roleLabel}</span>
        <DownOutlined className="agent-flow-llm-prompt-messages__role-icon" />
      </button>
    </Dropdown>
  );
}

export function LlmPromptMessagesField({
  value,
  options,
  onChange
}: LlmPromptMessagesFieldProps) {
  const [draggingIndex, setDraggingIndex] = useState<number | null>(null);
  const { systemMessage, dynamicMessages, orderedMessages } =
    normalizeMessageGroups(value);

  function addMessage() {
    onChange([
      ...orderedMessages,
      createPromptMessage('user', orderedMessages.length)
    ]);
  }

  function updateRole(index: number, role: LlmPromptMessageRole) {
    if (index === 0 || role === 'system') {
      return;
    }

    onChange(updateAt(orderedMessages, index, { role }));
  }

  function updateContent(index: number, nextValue: string) {
    onChange(
      orderedMessages.map((message, messageIndex) =>
        messageIndex === index
          ? {
              ...message,
              content: { kind: 'templated_text', value: nextValue }
            }
          : message
      )
    );
  }

  function removeMessage(index: number) {
    if (index === 0) {
      return;
    }

    onChange(
      orderedMessages.filter((_, messageIndex) => messageIndex !== index)
    );
  }

  function handleDrop(targetIndex: number) {
    if (draggingIndex === null || draggingIndex === 0 || targetIndex === 0) {
      setDraggingIndex(null);
      return;
    }

    onChange(moveItem(orderedMessages, draggingIndex, targetIndex));
    setDraggingIndex(null);
  }

  function renderPromptMessage(message: LlmPromptMessage, index: number) {
    const isSystemMessage = index === 0 && message.role === 'system';
    const isDraggableMessage = !isSystemMessage;
    const rowClassName = [
      'agent-flow-llm-prompt-messages__row',
      isSystemMessage ? 'agent-flow-llm-prompt-messages__row--fixed' : null,
      isDraggableMessage
        ? 'agent-flow-llm-prompt-messages__row--draggable'
        : null
    ]
      .filter(Boolean)
      .join(' ');
    const roleLabel = message.role.toUpperCase();

    return (
      <div
        key={message.id}
        className={rowClassName}
        data-testid={`llm-prompt-message-row-${message.id}`}
        onDragOver={(event) => event.preventDefault()}
        onDrop={() => handleDrop(index)}
      >
        <div className="agent-flow-llm-prompt-messages__body">
          <TemplatedTextField
            ariaLabel={i18nText("agentFlow", "auto.message_content", { value1: roleLabel })}
            draggable={isDraggableMessage}
            dragLabel={i18nText("agentFlow", "auto.drag_drop_sort_messages", { value1: roleLabel })}
            label={roleLabel}
            labelContent={
              isSystemMessage ? (
                <Typography.Text
                  strong
                  className="agent-flow-templated-text-field__label"
                >
                  SYSTEM
                </Typography.Text>
              ) : (
                <PromptMessageRoleSelect
                  ariaLabel={roleLabel + i18nText("agentFlow", "auto.message_role")}
                  value={message.role}
                  onChange={(role) => updateRole(index, role)}
                />
              )
            }
            toolbarExtraActions={
              isSystemMessage ? null : (
                <Button
                  aria-label={i18nText("agentFlow", "auto.delete_message", { value1: roleLabel })}
                  className="agent-flow-templated-text-field__action"
                  danger
                  icon={<DeleteOutlined />}
                  size="small"
                  type="text"
                  onClick={() => removeMessage(index)}
                />
              )
            }
            options={options}
            placeholder={i18nText("agentFlow", "auto.enter_text_enter_reference_variable")}
            value={message.content.value}
            onChange={(nextValue) => updateContent(index, nextValue)}
            onDragEnd={() => setDraggingIndex(null)}
            onDragStart={() => setDraggingIndex(index)}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="agent-flow-llm-prompt-messages">
      <div className="agent-flow-llm-prompt-messages__header">
        <Typography.Text className="agent-flow-node-detail__section-subtitle">
          {i18nText("agentFlow", "auto.contextual_messages_sent_model_sequence")}</Typography.Text>
      </div>

      <div className="agent-flow-llm-prompt-messages__list">
        {renderPromptMessage(systemMessage, 0)}
        <div
          className="agent-flow-llm-prompt-messages__dynamic-list"
          data-testid="llm-prompt-message-dynamic-list"
        >
          {dynamicMessages.map((message, dynamicIndex) =>
            renderPromptMessage(message, dynamicIndex + 1)
          )}
          <Button
            aria-label={i18nText("agentFlow", "auto.add_new_message")}
            className="agent-flow-llm-prompt-messages__add-message"
            icon={<PlusOutlined />}
            size="small"
            type="dashed"
            onClick={addMessage}
          >
            {i18nText("agentFlow", "auto.add_new_message")}</Button>
        </div>
      </div>
    </div>
  );
}
