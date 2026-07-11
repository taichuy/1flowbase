import { Button } from 'antd';
import type { ComponentProps } from 'react';

import './frontstage-node-action-button.css';

type FrontstageNodeActionButtonProps = Omit<
  ComponentProps<typeof Button>,
  'className' | 'size'
> & {
  className?: string;
};

export function FrontstageNodeActionButton({
  className,
  ...props
}: FrontstageNodeActionButtonProps) {
  return (
    <Button
      {...props}
      className={[
        'frontstage-node-action-button',
        className
      ]
        .filter(Boolean)
        .join(' ')}
      size="small"
    />
  );
}
