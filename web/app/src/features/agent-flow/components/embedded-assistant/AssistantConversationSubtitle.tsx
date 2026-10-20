import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';

import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';

import { App, Button, Tooltip } from 'antd';

import { i18nText } from '../../../../shared/i18n/text';

import { useClipboardCopy } from '../../../../shared/ui/clipboard/use-clipboard-copy';

export function AssistantConversationSubtitle({
  conversationId,
  flowName
}: {
  conversationId: string | null;
  flowName?: string;
}) {
  const { message } = App.useApp();
  const { copied, copy } = useClipboardCopy();

  async function copyConversationId() {
    if (!conversationId) {
      return;
    }
    try {
      await copy(conversationId);
      message.success(
        i18nText('appShell', 'auto.assistant_conversation_id_copied')
      );
    } catch {
      message.error(
        i18nText('appShell', 'auto.assistant_copy_conversation_id_failed')
      );
    }
  }

  return (
    <span className="embedded-agent-assistant-preview__subtitle">
      {flowName ? (
        <span className="embedded-agent-assistant-preview__flow-name">
          {flowName}
        </span>
      ) : null}
      {conversationId ? (
        <span className="embedded-agent-assistant-preview__conversation-id">
          <Tooltip title={conversationId}>
            <span className="embedded-agent-assistant-preview__conversation-id-value">
              {conversationId}
            </span>
          </Tooltip>
          <Tooltip
            title={i18nText('appShell', 'auto.assistant_copy_conversation_id')}
          >
            <Button
              aria-label={i18nText(
                'appShell',
                'auto.assistant_copy_conversation_id'
              )}
              className="embedded-agent-assistant-preview__conversation-id-copy"
              icon={copied ? <CheckOutlined /> : <CopyOutlined />}
              size="small"
              type="text"
              onClick={() => void copyConversationId()}
            />
          </Tooltip>
        </span>
      ) : null}
    </span>
  );
}
