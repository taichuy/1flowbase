import type { ReactNode } from 'react';
import { Typography } from 'antd';

export function CollectionFieldHeader({
  title,
  description,
  actions
}: {
  title: string;
  description?: ReactNode;
  actions: ReactNode;
}) {
  return (
    <div
      className="agent-flow-collection-field-header"
      data-testid="agent-flow-collection-field-header"
    >
      <div className="agent-flow-collection-field-header__top">
        <Typography.Title
          className="agent-flow-node-detail__section-title"
          level={5}
        >
          {title}
        </Typography.Title>
        <div className="agent-flow-collection-field-header__actions">
          {actions}
        </div>
      </div>
      {description ? (
        <Typography.Text className="agent-flow-node-detail__section-subtitle">
          {description}
        </Typography.Text>
      ) : null}
    </div>
  );
}
