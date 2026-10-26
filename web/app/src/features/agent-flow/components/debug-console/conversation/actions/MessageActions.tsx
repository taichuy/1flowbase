import { Space } from 'antd';
import type { ReactNode } from 'react';
import { i18nText } from '../../../../../../shared/i18n/text';
import '../debug-message.css';

export function MessageActions({
  children,
  start
}: {
  children: ReactNode;
  start?: ReactNode;
}) {
  return (
    <fieldset
      aria-label={i18nText('agentFlow', 'auto.output_action')}
      className="agent-flow-editor__debug-message-action-row"
    >
      {start ? (
        <span className="agent-flow-editor__debug-message-action-start">
          {start}
        </span>
      ) : null}
      <Space className="agent-flow-editor__debug-message-actions" size={8} wrap>
        {children}
      </Space>
    </fieldset>
  );
}
