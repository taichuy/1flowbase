import {
  ArrowDownOutlined,
  ArrowUpOutlined,
  CodeOutlined,
  DeleteOutlined,
  EditOutlined,
  HolderOutlined,
  MoreOutlined,
  SettingOutlined
} from '@ant-design/icons';
import { Button, Divider, Popconfirm, Popover, Space, Typography, message } from 'antd';
import type { CSSProperties, FC } from 'react';
import { useState } from 'react';
import { i18nText } from '../../../shared/i18n/text';
import { FRONTSTAGE_DESIGN_BLUE } from '../lib/design-mode-theme';

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

const toolbarStyle: CSSProperties = {
  position: 'absolute',
  top: 10,
  right: 10,
  zIndex: 10,
  background: '#fff',
  border: `1px solid ${FRONTSTAGE_DESIGN_BLUE.toolbarBorder}`,
  borderRadius: 8,
  boxShadow: FRONTSTAGE_DESIGN_BLUE.toolbarShadow,
  padding: 3,
  transition: 'opacity 0.15s ease'
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
  const [isMovePopoverOpen, setIsMovePopoverOpen] = useState(false);
  const [isMorePopoverOpen, setIsMorePopoverOpen] = useState(false);
  const [isDeleteConfirmOpen, setIsDeleteConfirmOpen] = useState(false);

  const copyBlockUid = () => {
    if (typeof navigator !== 'undefined' && navigator.clipboard) {
      void navigator.clipboard.writeText(blockId).then(
        () => {
          void message.success(i18nText("frontstage", "auto.uid_copied"));
        },
        () => {
          void message.warning(i18nText("frontstage", "auto.copy_uid_failed"));
        }
      );
      return;
    }

    void message.warning(i18nText("frontstage", "auto.auto_copy_unsupported"));
  };

  const menuButtonStyle: CSSProperties = {
    width: '100%',
    justifyContent: 'flex-start',
    textAlign: 'left'
  };

  const moveContent = (
    <Space direction="vertical" size={4} style={{ minWidth: 132 }}>
      <Button
        size="small"
        type="text"
        block
        icon={<ArrowUpOutlined />}
        aria-label={i18nText("frontstage", "auto.move_block_up")}
        disabled={disabled || !canMoveUp}
        onClick={(e) => {
          e.stopPropagation();
          setIsMovePopoverOpen(false);
          onMoveUp();
        }}
        style={menuButtonStyle}
      >
        {i18nText("frontstage", "auto.move_block_up")}</Button>
      <Button
        size="small"
        type="text"
        block
        icon={<ArrowDownOutlined />}
        aria-label={i18nText("frontstage", "auto.move_block_down")}
        disabled={disabled || !canMoveDown}
        onClick={(e) => {
          e.stopPropagation();
          setIsMovePopoverOpen(false);
          onMoveDown();
        }}
        style={menuButtonStyle}
      >
        {i18nText("frontstage", "auto.move_block_down")}</Button>
    </Space>
  );

  const moreContent = (
    <div
      style={{ minWidth: 168 }}
      onClick={(event) => {
        event.stopPropagation();
      }}
    >
      <Space direction="vertical" size={2} style={{ width: '100%' }}>
        <Button
          size="small"
          type="text"
          block
          icon={<EditOutlined />}
          aria-label={i18nText("frontstage", "auto.title_and_description")}
          disabled={disabled}
          onClick={(event) => {
            event.stopPropagation();
            setIsMorePopoverOpen(false);
            onConfigure();
          }}
          style={menuButtonStyle}
        >
          {i18nText("frontstage", "auto.title_and_description")}</Button>
        <Button
          size="small"
          type="text"
          block
          icon={<SettingOutlined />}
          aria-label={i18nText("frontstage", "auto.block_linkage_rules")}
          disabled={disabled}
          onClick={(event) => {
            event.stopPropagation();
            setIsMorePopoverOpen(false);
            onConfigure();
          }}
          style={menuButtonStyle}
        >
          {i18nText("frontstage", "auto.block_linkage_rules")}</Button>
        <Button
          size="small"
          type="text"
          block
          icon={<SettingOutlined />}
          aria-label={i18nText("frontstage", "auto.block_height")}
          disabled={disabled}
          onClick={(event) => {
            event.stopPropagation();
            setIsMorePopoverOpen(false);
            onConfigure();
          }}
          style={menuButtonStyle}
        >
          {i18nText("frontstage", "auto.block_height")}</Button>
        <Button
          size="small"
          type="text"
          block
          icon={<CodeOutlined />}
          aria-label="Write JavaScript"
          disabled={disabled}
          onClick={(event) => {
            event.stopPropagation();
            setIsMorePopoverOpen(false);
            onEditCode();
          }}
          style={menuButtonStyle}
        >
          Write JavaScript
        </Button>
        <Divider style={{ margin: '4px 0' }} />
        <Button
          size="small"
          type="text"
          block
          aria-label={i18nText("frontstage", "auto.save_as_template")}
          disabled
          style={menuButtonStyle}
        >
          {i18nText("frontstage", "auto.save_as_template")}</Button>
        <Button
          size="small"
          type="text"
          block
          aria-label={i18nText("frontstage", "auto.copy_uid")}
          disabled={disabled}
          onClick={(event) => {
            event.stopPropagation();
            setIsMorePopoverOpen(false);
            copyBlockUid();
          }}
          style={menuButtonStyle}
        >
          {i18nText("frontstage", "auto.copy_uid")}</Button>
        <Popconfirm
          title={i18nText("frontstage", "auto.confirm_delete_this_block")}
          trigger="click"
          okText={i18nText("frontstage", "auto.delete")}
          cancelText={i18nText("frontstage", "auto.cancel")}
          okButtonProps={{ 'aria-label': i18nText("frontstage", "auto.confirm_delete_block") }}
          open={isDeleteConfirmOpen}
          onOpenChange={(open) => {
            setIsDeleteConfirmOpen(open);
            if (open) {
              setIsMorePopoverOpen(true);
            }
          }}
          destroyOnHidden
          onConfirm={() => {
            setIsDeleteConfirmOpen(false);
            setIsMorePopoverOpen(false);
            onDelete();
          }}
          onCancel={() => setIsDeleteConfirmOpen(false)}
        >
          <Button
            size="small"
            type="text"
            block
            danger
            icon={<DeleteOutlined />}
            aria-label={i18nText("frontstage", "auto.delete_block")}
            disabled={disabled}
            onClick={(event) => {
              event.stopPropagation();
              setIsDeleteConfirmOpen(true);
              setIsMorePopoverOpen(true);
            }}
            style={menuButtonStyle}
          >
            {i18nText("frontstage", "auto.delete")}</Button>
        </Popconfirm>
      </Space>
      <Typography.Text type="secondary" style={{ display: 'block', marginTop: 8, fontSize: 11 }}>
        {i18nText("frontstage", "auto.save_as_template_unavailable")}</Typography.Text>
    </div>
  );

  return (
    <div
      style={{
        ...toolbarStyle,
        opacity: isVisible ? 1 : 0,
        pointerEvents: isVisible ? 'auto' : 'none'
      }}
      onClick={(event) => event.stopPropagation()}
    >
      <Space size={2}>
        <Popover
          content={moveContent}
          trigger="click"
          placement="bottomRight"
          open={isMovePopoverOpen}
          destroyOnHidden
          onOpenChange={setIsMovePopoverOpen}
        >
          <Button
            size="small"
            type="text"
            icon={<HolderOutlined />}
            disabled={disabled}
            aria-label={i18nText("frontstage", "auto.move_or_sort_block")}
          />
        </Popover>
        <Button
          size="small"
          type="text"
          icon={<EditOutlined />}
          disabled={disabled}
          onClick={(e) => {
            e.stopPropagation();
            onEditCode();
          }}
          aria-label={i18nText("frontstage", "auto.edit_block")}
        />
        <Popover
          content={moreContent}
          trigger="click"
          placement="bottomRight"
          open={isMorePopoverOpen}
          destroyOnHidden
          onOpenChange={(open) => {
            if (!open && isDeleteConfirmOpen) {
              return;
            }
            setIsMorePopoverOpen(open);
          }}
        >
          <Button
            size="small"
            type="text"
            icon={<MoreOutlined />}
            disabled={disabled}
            aria-label={i18nText("frontstage", "auto.more_block_operations")}
          />
        </Popover>
      </Space>
    </div>
  );
};
