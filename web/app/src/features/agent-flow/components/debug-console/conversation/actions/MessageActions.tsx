import { Space } from 'antd';
import type { ReactNode } from 'react';
import { i18nText } from '../../../../../../shared/i18n/text';
import '../debug-message.css';

export function MessageActions({ children }: { children: ReactNode }) {
  return (
    <fieldset
      aria-label={i18nText('agentFlow', 'auto.output_action')}
      className="agent-flow-editor__debug-message-action-row"
    >
      <Space className="agent-flow-editor__debug-message-actions" size={8} wrap>
        {children}
      </Space>
    </fieldset>
  );
}
