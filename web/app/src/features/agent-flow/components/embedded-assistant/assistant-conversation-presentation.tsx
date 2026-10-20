import {
  type ConsoleAssistantConversationPage,
  type ConsoleAssistantConversationSummary
} from '@1flowbase/api-client';
import ClockCircleOutlined from '@ant-design/icons/es/icons/ClockCircleOutlined';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';

import ExclamationCircleOutlined from '@ant-design/icons/es/icons/ExclamationCircleOutlined';
import LoadingOutlined from '@ant-design/icons/es/icons/LoadingOutlined';

import { Tooltip } from 'antd';

import { i18nText } from '../../../../shared/i18n/text';

export function assistantConversationKey(item: {
  conversation_id: string | null;
  legacy_flow_run_id: string | null;
}) {
  return item.conversation_id
    ? `conversation:${item.conversation_id}`
    : `legacy:${item.legacy_flow_run_id}`;
}

export function assistantConversationGroup(updatedAt: string) {
  const date = new Date(updatedAt);
  return Number.isNaN(date.getTime())
    ? i18nText('appShell', 'auto.assistant_history')
    : assistantConversationDateFormatter.format(date);
}

export function upsertAssistantConversation(
  page: ConsoleAssistantConversationPage,
  item: ConsoleAssistantConversationSummary,
  eventType: 'conversation.created' | 'conversation.updated'
): ConsoleAssistantConversationPage {
  const itemKey = assistantConversationKey(item);
  const exists = page.items.some(
    (candidate) => assistantConversationKey(candidate) === itemKey
  );
  const items = [
    item,
    ...page.items.filter(
      (candidate) => assistantConversationKey(candidate) !== itemKey
    )
  ].sort((left, right) => {
    const updatedAtOrder = right.updated_at.localeCompare(left.updated_at);
    return updatedAtOrder !== 0
      ? updatedAtOrder
      : assistantConversationKey(right).localeCompare(
          assistantConversationKey(left)
        );
  });
  return {
    ...page,
    items,
    total:
      !exists && eventType === 'conversation.created'
        ? page.total + 1
        : page.total
  };
}

export function assistantRunStatusIndicator(status: string | null) {
  const commonProps = {
    'data-assistant-run-status': status ?? undefined
  };
  if (['queued', 'running', 'waiting_callback'].includes(status ?? '')) {
    const label = i18nText('appShell', 'auto.assistant_status_running');
    return (
      <Tooltip title={label}>
        <span
          {...commonProps}
          aria-label={label}
          className="embedded-agent-assistant-preview__history-item-status embedded-agent-assistant-preview__history-item-status--running"
        >
          <LoadingOutlined spin />
        </span>
      </Tooltip>
    );
  }
  if (['waiting_human', 'paused'].includes(status ?? '')) {
    const label = i18nText('appShell', 'auto.assistant_status_waiting');
    return (
      <Tooltip title={label}>
        <span
          {...commonProps}
          aria-label={label}
          className="embedded-agent-assistant-preview__history-item-status embedded-agent-assistant-preview__history-item-status--waiting"
        >
          <ClockCircleOutlined />
        </span>
      </Tooltip>
    );
  }
  if (['failed', 'incomplete'].includes(status ?? '')) {
    const label = i18nText('appShell', 'auto.assistant_status_failed');
    return (
      <Tooltip title={label}>
        <span
          {...commonProps}
          aria-label={label}
          className="embedded-agent-assistant-preview__history-item-status embedded-agent-assistant-preview__history-item-status--failed"
        >
          <ExclamationCircleOutlined />
        </span>
      </Tooltip>
    );
  }
  if (status === 'cancelled') {
    const label = i18nText('appShell', 'auto.assistant_status_cancelled');
    return (
      <Tooltip title={label}>
        <span
          {...commonProps}
          aria-label={label}
          className="embedded-agent-assistant-preview__history-item-status embedded-agent-assistant-preview__history-item-status--cancelled"
        >
          <CloseOutlined />
        </span>
      </Tooltip>
    );
  }
  return null;
}

const assistantConversationDateFormatter = new Intl.DateTimeFormat(undefined, {
  month: 'short',
  day: 'numeric'
});

