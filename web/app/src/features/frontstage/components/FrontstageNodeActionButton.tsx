import { Button } from 'antd';
import { forwardRef, type ComponentProps, type ComponentRef } from 'react';

import './frontstage-node-action-button.css';

type FrontstageNodeActionButtonProps = Omit<
  ComponentProps<typeof Button>,
  'className' | 'size'
> & {
  className?: string;
};

export const FrontstageNodeActionButton = forwardRef<
  ComponentRef<typeof Button>,
  FrontstageNodeActionButtonProps
>(function FrontstageNodeActionButton({ className, ...props }, ref) {
  return (
    <Button
      {...props}
      ref={ref}
      className={['frontstage-node-action-button', className]
        .filter(Boolean)
        .join(' ')}
      size="small"
    />
  );
});
