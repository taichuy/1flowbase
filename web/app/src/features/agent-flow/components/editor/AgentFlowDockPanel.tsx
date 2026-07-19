import { CloseOutlined } from '@ant-design/icons';
import { Button, Space, Typography } from 'antd';
import type { ReactNode } from 'react';

import { SchemaDockPanel } from '../../../../shared/schema-ui/v1/overlay-shell/SchemaDockPanel';
import { i18nText } from '../../../../shared/i18n/text';

interface AgentFlowDockPanelProps {
  actions?: ReactNode;
  ariaLabel?: string;
  bodyClassName?: string;
  children: ReactNode;
  className?: string;
  closeLabel?: string;
  subtitle?: ReactNode;
  title: string;
  onClose: () => void;
}

export function AgentFlowDockPanel({
  actions,
  ariaLabel,
  bodyClassName,
  children,
  className,
  closeLabel,
  subtitle,
  title,
  onClose
}: AgentFlowDockPanelProps) {
  const shellSchema = {
    schemaVersion: '1.0.0',
    shellType: 'dock_panel',
    title: ariaLabel ?? title
  } as const;

  return (
    <SchemaDockPanel
      bodyClassName={['agent-flow-editor__dock-panel-body', bodyClassName]
        .filter(Boolean)
        .join(' ')}
      className={['agent-flow-editor__dock-panel', className]
        .filter(Boolean)
        .join(' ')}
      headerless
      schema={shellSchema}
    >
      <div className="agent-flow-editor__dock-panel-header">
        <div className="agent-flow-editor__dock-panel-title">
          <div className="agent-flow-editor__dock-panel-title-stack">
            <Typography.Text strong>{title}</Typography.Text>
            {subtitle ? (
              <Typography.Text type="secondary">{subtitle}</Typography.Text>
            ) : null}
          </div>
        </div>
        <Space size={4} wrap>
          {actions}
          <Button
            aria-label={closeLabel ?? i18nText("agentFlow", "auto.close", { value1: title })}
            icon={<CloseOutlined />}
            size="small"
            type="text"
            onClick={onClose}
          />
        </Space>
      </div>
      {children}
    </SchemaDockPanel>
  );
}
