import { DeleteOutlined, PlusOutlined, SettingOutlined, SwapLeftOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Button, Input, Modal, Popconfirm, Space, Tabs, Tooltip, Typography } from 'antd';
import type { FC } from 'react';
import { useMemo, useState } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { i18nText } from '../../../shared/i18n/text';
import { FRONTSTAGE_DESIGN_BLUE } from '../lib/design-mode-theme';
import {
  createFrontstagePageTab,
  deleteFrontstagePageTab,
  fetchFrontstagePageTabs,
  frontstagePageTabsQueryKey,
  moveFrontstagePageTab,
  renameFrontstagePageTab,
  type FrontstagePageTab
} from '../api/page-tabs';

interface FrontstagePageTabsProps {
  workspaceId: string;
  pageId: string;
  tabId: string;
  isDesignMode: boolean;
  onNavigateTab: (tabId: string) => void;
}

function nextRank(tabs: FrontstagePageTab[]): string {
  return String((tabs.length + 1) * 1000).padStart(6, '0');
}

function previousRank(index: number): string {
  return index <= 1 ? '000000' : String((index - 1) * 1000 + 500).padStart(6, '0');
}

function tabLabelText(tab: FrontstagePageTab): string {
  return tab.title?.trim() || i18nText('frontstage', 'auto.unnamed_page_tab');
}

interface DesignTabLabelProps {
  tab: FrontstagePageTab;
  isActive: boolean;
  onConfigure: (tab: FrontstagePageTab) => void;
}

/**
 * Tab label with the shared design-mode hover affordance: hovering reveals a
 * configure entry that opens the tab settings dialog, mirroring the page-title
 * and block hover toolbars.
 */
const DesignTabLabel: FC<DesignTabLabelProps> = ({ tab, isActive, onConfigure }) => {
  const [isHovered, setIsHovered] = useState(false);

  return (
    <span
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 4,
        padding: '0 4px',
        borderRadius: 6,
        color: isHovered || isActive ? FRONTSTAGE_DESIGN_BLUE.primary : undefined,
        background: isHovered ? FRONTSTAGE_DESIGN_BLUE.bgHover : 'transparent',
        boxShadow: isHovered ? FRONTSTAGE_DESIGN_BLUE.halo : 'none',
        transition: 'background 0.15s ease, box-shadow 0.15s ease, color 0.15s ease'
      }}
    >
      {tabLabelText(tab)}
      <Tooltip title={i18nText('frontstage', 'design.configure_tab')}>
        <Button
          size="small"
          type="text"
          aria-label={i18nText('frontstage', 'design.configure_tab')}
          icon={<SettingOutlined />}
          style={{
            opacity: isHovered || isActive ? 1 : 0,
            pointerEvents: isHovered || isActive ? 'auto' : 'none',
            color: FRONTSTAGE_DESIGN_BLUE.primary,
            width: 20,
            height: 20,
            minWidth: 20
          }}
          onClick={(event) => {
            event.stopPropagation();
            onConfigure(tab);
          }}
        />
      </Tooltip>
    </span>
  );
};

export function FrontstagePageTabs({
  workspaceId,
  pageId,
  tabId,
  isDesignMode,
  onNavigateTab
}: FrontstagePageTabsProps) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [configuringTab, setConfiguringTab] = useState<FrontstagePageTab | null>(null);
  const [configTitle, setConfigTitle] = useState('');
  const queryKey = frontstagePageTabsQueryKey(workspaceId, pageId);
  const tabsQuery = useQuery({
    queryKey,
    queryFn: () => fetchFrontstagePageTabs(workspaceId, pageId),
    retry: false
  });
  const tabs = useMemo(
    () => [...(tabsQuery.data ?? [])].sort((left, right) => left.rank.localeCompare(right.rank)),
    [tabsQuery.data]
  );

  const runMutation = async <T,>(operation: () => Promise<T>): Promise<T | null> => {
    setError(null);
    try {
      const result = await operation();
      await queryClient.invalidateQueries({ queryKey });
      return result;
    } catch (caughtError) {
      setError(
        caughtError instanceof Error
          ? caughtError.message
          : i18nText('frontstage', 'auto.page_tab_operation_failed')
      );
      return null;
    }
  };

  const createMutation = useMutation({
    mutationFn: () =>
      createFrontstagePageTab(
        workspaceId,
        pageId,
        { title: null, rank: nextRank(tabs) },
        csrfToken ?? ''
      ),
    onSuccess: (createdTab) => {
      onNavigateTab(createdTab.id);
    }
  });

  if (!isDesignMode && tabs.length <= 1) {
    return null;
  }

  const configuringIndex = configuringTab
    ? tabs.findIndex((tab) => tab.id === configuringTab.id)
    : -1;

  const openConfigureDialog = (tab: FrontstagePageTab) => {
    setConfiguringTab(tab);
    setConfigTitle(tab.title ?? '');
  };

  const closeConfigureDialog = () => {
    setConfiguringTab(null);
    setConfigTitle('');
  };

  const handleRename = () => {
    if (!configuringTab) return;
    const title = configTitle.trim();
    void runMutation(() =>
      renameFrontstagePageTab(
        workspaceId,
        pageId,
        configuringTab.id,
        { title: title || null },
        csrfToken ?? ''
      )
    ).then((result) => {
      if (result !== null) {
        closeConfigureDialog();
      }
    });
  };

  const handleMoveForward = () => {
    if (!configuringTab || configuringIndex <= 0) return;
    void runMutation(() =>
      moveFrontstagePageTab(
        workspaceId,
        pageId,
        configuringTab.id,
        { rank: previousRank(configuringIndex) },
        csrfToken ?? ''
      )
    );
  };

  const handleDelete = () => {
    if (!configuringTab) return;
    const nextTab =
      tabs[configuringIndex + 1] ?? tabs[configuringIndex - 1] ?? null;
    void runMutation(() =>
      deleteFrontstagePageTab(workspaceId, pageId, configuringTab.id, csrfToken ?? '')
    ).then((deleted) => {
      closeConfigureDialog();
      if (deleted !== null && nextTab && configuringTab.id === tabId) {
        onNavigateTab(nextTab.id);
      }
    });
  };

  return (
    <Space direction="vertical" size={4} style={{ width: '100%' }}>
      <Tabs
        activeKey={tabId}
        onChange={onNavigateTab}
        items={tabs.map((tab) => ({
          key: tab.id,
          label: isDesignMode ? (
            <DesignTabLabel
              tab={tab}
              isActive={tab.id === tabId}
              onConfigure={openConfigureDialog}
            />
          ) : (
            tabLabelText(tab)
          )
        }))}
        tabBarExtraContent={
          isDesignMode ? (
            <Tooltip title={i18nText('frontstage', 'auto.create_page_tab')}>
              <Button
                size="small"
                type="dashed"
                icon={<PlusOutlined />}
                aria-label={i18nText('frontstage', 'auto.create_page_tab')}
                disabled={!csrfToken}
                onClick={() => void runMutation(() => createMutation.mutateAsync())}
              >
                {i18nText('frontstage', 'auto.create_page_tab')}
              </Button>
            </Tooltip>
          ) : undefined
        }
      />
      {error ? <Typography.Text type="danger">{error}</Typography.Text> : null}
      <Modal
        open={configuringTab !== null}
        title={i18nText('frontstage', 'design.tab_settings')}
        onCancel={closeConfigureDialog}
        footer={null}
        destroyOnHidden
      >
        <Space direction="vertical" size={16} style={{ width: '100%' }}>
          <Space.Compact style={{ width: '100%' }}>
            <Input
              aria-label={i18nText('frontstage', 'auto.page_tab_name')}
              value={configTitle}
              onChange={(event) => setConfigTitle(event.target.value)}
              placeholder={i18nText('frontstage', 'auto.page_tab_name')}
              onPressEnter={handleRename}
            />
            <Button type="primary" disabled={!csrfToken} onClick={handleRename}>
              {i18nText('frontstage', 'auto.rename_current_page_tab')}
            </Button>
          </Space.Compact>
          <Space wrap>
            <Button
              icon={<SwapLeftOutlined />}
              disabled={configuringIndex <= 0 || !csrfToken}
              onClick={handleMoveForward}
            >
              {i18nText('frontstage', 'auto.move_current_page_tab_forward')}
            </Button>
            <Popconfirm
              title={i18nText('frontstage', 'design.delete_tab_confirm')}
              okButtonProps={{ danger: true }}
              onConfirm={handleDelete}
              disabled={tabs.length <= 1 || !csrfToken}
            >
              <Button
                danger
                icon={<DeleteOutlined />}
                disabled={tabs.length <= 1 || !csrfToken}
              >
                {i18nText('frontstage', 'auto.delete_current_page_tab')}
              </Button>
            </Popconfirm>
          </Space>
          {tabs.length <= 1 ? (
            <Typography.Text type="secondary">
              {i18nText('frontstage', 'design.last_tab_hint')}
            </Typography.Text>
          ) : null}
        </Space>
      </Modal>
    </Space>
  );
}
