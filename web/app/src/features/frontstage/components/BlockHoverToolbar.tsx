import {
  ArrowDownOutlined,
  ArrowUpOutlined,
  CodeOutlined,
  DeleteOutlined,
  HolderOutlined,
  MoreOutlined,
  SettingOutlined
} from '@ant-design/icons';
import { App as AntdApp, Button, Divider, Tooltip, message } from 'antd';
import type { FC, MouseEvent } from 'react';
import { useState } from 'react';

import { i18nText } from '../../../shared/i18n/text';
import { copyTextToClipboard } from '../../../shared/ui/clipboard/copy-text';
import { FrontstageNodeActionButton } from './FrontstageNodeActionButton';

type BlockHoverToolbarProps = {
  blockId: string;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onConfigure: () => void;
  onEditCode: () => void;
  onDelete: () => void;
  canMoveUp: boolean;
  canMoveDown: boolean;
  isVisible: boolean;
  disabled?: boolean;
};

export const BlockHoverToolbar: FC<BlockHoverToolbarProps> = ({
  blockId,
  onMoveUp,
  onMoveDown,
  onConfigure,
  onEditCode,
  onDelete,
  canMoveUp,
  canMoveDown,
  isVisible,
  disabled = false
}) => {
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
          icon={<HolderOutlined />}
        />
      </Tooltip>
      <Tooltip title={i18nText('frontstage', 'auto.block_configuration')}>
        <FrontstageNodeActionButton
          aria-label={i18nText('frontstage', 'auto.block_configuration')}
          disabled={disabled}
          icon={<SettingOutlined />}
          onClick={(event) => {
            event.stopPropagation();
            onConfigure();
          }}
        />
      </Tooltip>
      <Tooltip title={i18nText('frontstage', 'auto.block_code')}>
        <FrontstageNodeActionButton
          aria-label={i18nText('frontstage', 'auto.block_code')}
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
          icon={<MoreOutlined />}
          onClick={(event) => {
            event.stopPropagation();
            setIsMoreMenuOpen((open) => !open);
          }}
        />
        {isMoreMenuOpen ? (
          <div
            className="frontstage-block-hover-actions__menu"
            role="menu"
          >
            <Button
              block
              type="text"
              role="menuitem"
              icon={<ArrowUpOutlined />}
              disabled={disabled || !canMoveUp}
              onClick={menuAction(onMoveUp)}
            >
              {i18nText('frontstage', 'auto.move_block_up')}
            </Button>
            <Button
              block
              type="text"
              role="menuitem"
              icon={<ArrowDownOutlined />}
              disabled={disabled || !canMoveDown}
              onClick={menuAction(onMoveDown)}
            >
              {i18nText('frontstage', 'auto.move_block_down')}
            </Button>
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
