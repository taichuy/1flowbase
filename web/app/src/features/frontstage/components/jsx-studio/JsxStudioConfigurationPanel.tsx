import { Descriptions } from 'antd';
import type { DescriptionsProps } from 'antd';
import type { ReactNode } from 'react';

export function JsxStudioConfigurationPanel({
  actions,
  items
}: {
  actions?: ReactNode;
  items: NonNullable<DescriptionsProps['items']>;
}) {
  return (
    <div className="frontstage-jsx-studio__configuration-panel frontstage-jsx-studio__resource-scroll">
      <Descriptions column={1} items={items} size="small" />
      {actions}
    </div>
  );
}
