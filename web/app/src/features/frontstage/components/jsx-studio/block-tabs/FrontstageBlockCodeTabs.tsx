import { CloseOutlined, LoadingOutlined } from '@ant-design/icons';
import { App, Tabs, Tooltip } from 'antd';

import { i18nText } from '../../../../../shared/i18n/text';
import type { FrontstageBlockCodeTabState } from './use-frontstage-block-tabs';

import './frontstage-block-code-tabs.css';

export function FrontstageBlockCodeTabs({
  tabs,
  activeBlockId,
  initialBlockId,
  onActivate,
  onClose
}: {
  tabs: FrontstageBlockCodeTabState[];
  activeBlockId: string;
  initialBlockId: string;
  onActivate: (blockId: string) => void;
  onClose: (blockId: string) => void;
}) {
  const { modal } = App.useApp();

  const requestClose = (blockId: string) => {
    const tab = tabs.find((candidate) => candidate.block_id === blockId);
    if (!tab || blockId === initialBlockId) return;
    if (tab.draft === tab.base_source) {
      onClose(blockId);
      return;
    }
    modal.confirm({
      title: i18nText('frontstage', 'auto.block_tab_close_dirty_title'),
      content: i18nText(
        'frontstage',
        'auto.block_tab_close_dirty_description'
      ),
      okText: i18nText('frontstage', 'auto.close'),
      cancelText: i18nText('frontstage', 'auto.cancel'),
      onOk: () => onClose(blockId)
    });
  };

  return (
    <div className="frontstage-block-code-tabs">
      <Tabs
        activeKey={activeBlockId}
        hideAdd
        size="small"
        type="editable-card"
        items={tabs.map((tab) => {
          const dirty = tab.draft !== tab.base_source;
          const title = tab.detail?.title ?? tab.block_id;
          return {
            key: tab.block_id,
            closable: tab.block_id !== initialBlockId,
            closeIcon: (
              <Tooltip title={i18nText('frontstage', 'auto.close')}>
                <CloseOutlined />
              </Tooltip>
            ),
            label: (
              <span className="frontstage-block-code-tabs__label">
                {tab.loading ? <LoadingOutlined spin /> : null}
                <span className="frontstage-block-code-tabs__title">
                  {title}
                </span>
                {dirty ? (
                  <span
                    aria-label={i18nText(
                      'frontstage',
                      'auto.block_tab_unsaved'
                    )}
                    className="frontstage-block-code-tabs__dirty"
                  />
                ) : null}
              </span>
            ),
            children: null
          };
        })}
        onChange={onActivate}
        onEdit={(targetKey, action) => {
          if (action === 'remove') requestClose(String(targetKey));
        }}
      />
    </div>
  );
}
