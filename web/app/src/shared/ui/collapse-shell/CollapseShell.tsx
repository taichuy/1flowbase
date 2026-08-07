import type { ReactNode } from 'react';

import { Collapse } from 'antd';
import type { CollapseProps } from 'antd';

import './collapse-shell.css';

export interface CollapseShellItem {
  key: string;
  header: ReactNode;
  children: ReactNode;
  collapsible?: 'header' | 'icon' | 'disabled';
  forceRender?: boolean;
  showArrow?: boolean;
}

export interface CollapseShellProps {
  items: CollapseShellItem[];
  className?: string;
  variant?: 'default' | 'compact';
  accordion?: CollapseProps['accordion'];
  activeKey?: CollapseProps['activeKey'];
  defaultActiveKey?: CollapseProps['defaultActiveKey'];
  destroyOnHidden?: CollapseProps['destroyOnHidden'];
  expandIconPosition?: CollapseProps['expandIconPosition'];
  onChange?: CollapseProps['onChange'];
}

export function CollapseShell({
  items,
  className,
  variant = 'default',
  accordion,
  activeKey,
  defaultActiveKey,
  destroyOnHidden,
  expandIconPosition,
  onChange
}: CollapseShellProps) {
  const shellClassName = ['collapse-shell', `collapse-shell--${variant}`, className]
    .filter(Boolean)
    .join(' ');

  return (
    <Collapse
      bordered={false}
      className={shellClassName}
      accordion={accordion}
      activeKey={activeKey}
      defaultActiveKey={defaultActiveKey}
      destroyOnHidden={destroyOnHidden}
      expandIconPlacement={expandIconPosition}
      onChange={onChange}
      items={items.map((item) => ({
        key: item.key,
        label: <div className="collapse-shell__header-content">{item.header}</div>,
        children: <div className="collapse-shell__content">{item.children}</div>,
        collapsible: item.collapsible,
        forceRender: item.forceRender,
        showArrow: item.showArrow
      }))}
    />
  );
}
