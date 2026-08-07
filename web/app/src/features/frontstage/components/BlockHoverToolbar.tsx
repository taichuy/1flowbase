import {
  CodeOutlined,
  DeleteOutlined,
  DragOutlined,
  MenuOutlined
} from '@ant-design/icons';
import { App as AntdApp, Button, Divider, Tooltip } from 'antd';
import type { FC, MouseEvent } from 'react';
import { useState } from 'react';

import { i18nText } from '../../../shared/i18n/text';
import { copyTextToClipboard } from '../../../shared/ui/clipboard/copy-text';
import { FrontstageNodeActionButton } from './FrontstageNodeActionButton';

type BlockHoverToolbarProps = {
  blockId: string;
  onEditCode: () => void;
  onDelete: () => void;
  isVisible: boolean;
  disabled?: boolean;
};

export const BlockHoverToolbar: FC<BlockHoverToolbarProps> = ({
  blockId,
  onEditCode,
  onDelete,
  isVisible,
  disabled = false
}) => {
  const { message } = AntdApp.useApp();
  const { modal } = AntdApp.useApp();
  const [isMoreMenuOpen, setIsMoreMenuOpen] = useState(false);

  const copyBlockUid = () => {
    void copyTextToClipboard(blockId).then(
      () => message.success(i18nText('frontstage', 'auto.uid_copied')),
      () => message.warning(i18nText('frontstage', 'auto.copy_uid_failed'))
    );
  };

  const menuAction = (action: () => void) => (event: MouseEvent) => {
    event.stopPropagation();
    setIsMoreMenuOpen(false);
    action();
  };

  return (
    <div
      className="frontstage-block-hover-actions"
      data-testid="frontstage-block-hover-actions"
      data-visible={isVisible ? 'true' : 'false'}
      onClick={(event) => event.stopPropagation()}
    >
      <Tooltip title={i18nText('frontstage', 'auto.move_or_sort_block')}>
        <FrontstageNodeActionButton
          aria-label={i18nText('frontstage', 'auto.move_or_sort_block')}
          className="frontstage-block-drag-handle"
          disabled={disabled}
          icon={<DragOutlined />}
        />
      </Tooltip>
      <Tooltip title={i18nText('frontstage', 'auto.edit_block')}>
        <FrontstageNodeActionButton
          aria-label={i18nText('frontstage', 'auto.edit_block')}
          disabled={disabled}
          icon={<CodeOutlined />}
          onClick={(event) => {
            event.stopPropagation();
            onEditCode();
          }}
        />
      </Tooltip>
      <span className="frontstage-block-hover-actions__more-trigger">
        <FrontstageNodeActionButton
          aria-expanded={isMoreMenuOpen}
          aria-haspopup="menu"
          aria-label={i18nText('frontstage', 'auto.more_block_operations')}
          disabled={disabled}
          icon={<MenuOutlined />}
          onClick={(event) => {
            event.stopPropagation();
            setIsMoreMenuOpen((open) => !open);
          }}
        />
        {isMoreMenuOpen ? (
          <div className="frontstage-block-hover-actions__menu" role="menu">
            <Button
              block
              type="text"
              role="menuitem"
              disabled={disabled}
              onClick={menuAction(copyBlockUid)}
            >
              {i18nText('frontstage', 'auto.copy_uid')}
            </Button>
            <Divider />
            <Button
              block
              danger
              type="text"
              role="menuitem"
              icon={<DeleteOutlined />}
              disabled={disabled}
              onClick={menuAction(() => {
                modal.confirm({
                  title: i18nText(
                    'frontstage',
                    'auto.confirm_delete_this_block'
                  ),
                  okText: i18nText('frontstage', 'auto.delete'),
                  cancelText: i18nText('frontstage', 'auto.cancel'),
                  okButtonProps: { danger: true },
                  onOk: onDelete
                });
              })}
            >
              {i18nText('frontstage', 'auto.delete')}
            </Button>
          </div>
        ) : null}
      </span>
    </div>
  );
};
