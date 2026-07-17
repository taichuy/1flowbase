import {
  DeleteOutlined,
  DragOutlined,
  MenuOutlined,
  PlusOutlined
} from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  ConfigProvider,
  Input,
  Popconfirm,
  Popover,
  Space,
  Tabs,
  Tooltip,
  Typography
} from 'antd';
import type { CSSProperties, DragEvent, ReactNode } from 'react';
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
import { FrontstageNodeActionButton } from './FrontstageNodeActionButton';
import './frontstage-page-tabs.css';

interface FrontstagePageTabsProps {
  workspaceId: string;
  pageId: string;
  tabId: string;
  isDesignMode: boolean;
  onNavigateTab: (tabId: string) => void;
  children: ReactNode;
}

const PAGE_TAB_DRAG_DATA_TYPE = 'application/x-frontstage-page-tab';
const FRONTSTAGE_DESIGN_TABS_THEME = {
  components: {
    Tabs: {
      itemHoverColor: FRONTSTAGE_DESIGN_BLUE.primary,
      itemActiveColor: FRONTSTAGE_DESIGN_BLUE.primaryStrong,
      itemSelectedColor: FRONTSTAGE_DESIGN_BLUE.primary,
      inkBarColor: FRONTSTAGE_DESIGN_BLUE.primary
    }
  }
};

function nextRank(tabs: FrontstagePageTab[]): string {
  return String((tabs.length + 1) * 1000).padStart(6, '0');
}

function tabLabelText(tab: FrontstagePageTab): string {
  return tab.title?.trim() || i18nText('frontstage', 'auto.unnamed_page_tab');
}

function createTabRankUpdates(
  tabs: FrontstagePageTab[],
  draggedTabId: string,
  targetTabId: string
): Array<{ tabId: string; rank: string }> {
  const draggedIndex = tabs.findIndex((tab) => tab.id === draggedTabId);
  const targetIndex = tabs.findIndex((tab) => tab.id === targetTabId);
  if (draggedIndex < 0 || targetIndex < 0 || draggedIndex === targetIndex) {
    return [];
  }

  const reorderedTabs = [...tabs];
  const [draggedTab] = reorderedTabs.splice(draggedIndex, 1);
  if (!draggedTab) {
    return [];
  }
  reorderedTabs.splice(targetIndex, 0, draggedTab);

  return reorderedTabs.flatMap((tab, index) => {
    const rank = tabs[index]?.rank;
    return rank && tab.rank !== rank ? [{ tabId: tab.id, rank }] : [];
  });
}

export function FrontstagePageTabs({
  workspaceId,
  pageId,
  tabId,
  isDesignMode,
  onNavigateTab,
  children
}: FrontstagePageTabsProps) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [configuringTab, setConfiguringTab] =
    useState<FrontstagePageTab | null>(null);
  const [configTitle, setConfigTitle] = useState('');
  const [draggedTabId, setDraggedTabId] = useState<string | null>(null);
  const [dropTargetTabId, setDropTargetTabId] = useState<string | null>(null);
  const queryKey = frontstagePageTabsQueryKey(workspaceId, pageId);
  const tabsQuery = useQuery({
    queryKey,
    queryFn: () => fetchFrontstagePageTabs(workspaceId, pageId),
    retry: false
  });
  const tabs = useMemo(
    () =>
      [...(tabsQuery.data ?? [])].sort((left, right) =>
        left.rank.localeCompare(right.rank)
      ),
    [tabsQuery.data]
  );

  const runMutation = async <T,>(
    operation: () => Promise<T>
  ): Promise<T | null> => {
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
    return <>{children}</>;
  }

  const configuringIndex = configuringTab
    ? tabs.findIndex((tab) => tab.id === configuringTab.id)
    : -1;

  const openConfigurePopover = (tab: FrontstagePageTab) => {
    setConfiguringTab(tab);
    setConfigTitle(tab.title ?? '');
  };

  const closeConfigurePopover = () => {
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
        closeConfigurePopover();
      }
    });
  };

  const handleDelete = () => {
    if (!configuringTab) return;
    const nextTab =
      tabs[configuringIndex + 1] ?? tabs[configuringIndex - 1] ?? null;
    void runMutation(() =>
      deleteFrontstagePageTab(
        workspaceId,
        pageId,
        configuringTab.id,
        csrfToken ?? ''
      )
    ).then((deleted) => {
      closeConfigurePopover();
      if (deleted !== null && nextTab && configuringTab.id === tabId) {
        onNavigateTab(nextTab.id);
      }
    });
  };

  const resetDragState = () => {
    setDraggedTabId(null);
    setDropTargetTabId(null);
  };

  const handleTabDrop = (
    event: DragEvent<HTMLElement>,
    targetTabId: string
  ) => {
    event.preventDefault();
    event.stopPropagation();
    const sourceTabId =
      draggedTabId || event.dataTransfer.getData(PAGE_TAB_DRAG_DATA_TYPE);
    resetDragState();
    if (!sourceTabId || !csrfToken) {
      return;
    }

    const rankUpdates = createTabRankUpdates(tabs, sourceTabId, targetTabId);
    if (rankUpdates.length === 0) {
      return;
    }

    void runMutation(() =>
      Promise.all(
        rankUpdates.map((update) =>
          moveFrontstagePageTab(
            workspaceId,
            pageId,
            update.tabId,
            { rank: update.rank },
            csrfToken
          )
        )
      )
    );
  };

  const tabSettingsContent = configuringTab ? (
    <div
      className="frontstage-page-tabs__settings"
      onClick={(event) => event.stopPropagation()}
    >
      <Space direction="vertical" size={12} style={{ width: '100%' }}>
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
        <Popconfirm
          title={i18nText('frontstage', 'design.delete_tab_confirm')}
          okButtonProps={{ danger: true }}
          onConfirm={handleDelete}
          disabled={tabs.length <= 1 || !csrfToken}
        >
          <Button
            block
            danger
            type="text"
            icon={<DeleteOutlined />}
            disabled={tabs.length <= 1 || !csrfToken}
          >
            {i18nText('frontstage', 'auto.delete_current_page_tab')}
          </Button>
        </Popconfirm>
        {tabs.length <= 1 ? (
          <Typography.Text type="secondary">
            {i18nText('frontstage', 'design.last_tab_hint')}
          </Typography.Text>
        ) : null}
      </Space>
    </div>
  ) : null;

  return (
    <div className="frontstage-page-tabs">
      <ConfigProvider
        theme={isDesignMode ? FRONTSTAGE_DESIGN_TABS_THEME : undefined}
      >
        <Tabs
          activeKey={tabId}
          onChange={onNavigateTab}
          items={tabs.map((tab) => ({
            key: tab.id,
            label: isDesignMode ? (
              <span
                className={[
                  'frontstage-page-tabs__label',
                  configuringTab?.id === tab.id
                    ? 'frontstage-page-tabs__label--configuring'
                    : null,
                  dropTargetTabId === tab.id
                    ? 'frontstage-page-tabs__label--drop-target'
                    : null
                ]
                  .filter(Boolean)
                  .join(' ')}
                style={
                  dropTargetTabId === tab.id
                    ? {
                        borderColor: FRONTSTAGE_DESIGN_BLUE.dashed,
                        borderStyle: 'dashed',
                        background: FRONTSTAGE_DESIGN_BLUE.bgDashed
                      }
                    : undefined
                }
                data-testid={`frontstage-tab-label-${tab.id}`}
                onDragOver={(event) => {
                  if (!draggedTabId || draggedTabId === tab.id) {
                    return;
                  }
                  event.preventDefault();
                  event.dataTransfer.dropEffect = 'move';
                  setDropTargetTabId(tab.id);
                }}
                onDrop={(event) => handleTabDrop(event, tab.id)}
              >
                <span>{tabLabelText(tab)}</span>
                <span
                  className="frontstage-page-tabs__label-actions"
                  onClick={(event) => event.stopPropagation()}
                >
                  <Tooltip title={i18nText('frontstage', 'design.drag_handle')}>
                    <FrontstageNodeActionButton
                      aria-label={i18nText('frontstage', 'design.drag_handle')}
                      disabled={!csrfToken}
                      draggable={Boolean(csrfToken)}
                      icon={<DragOutlined />}
                      onClick={(event) => event.stopPropagation()}
                      onDragStart={(event) => {
                        event.stopPropagation();
                        event.dataTransfer.effectAllowed = 'move';
                        event.dataTransfer.setData(
                          PAGE_TAB_DRAG_DATA_TYPE,
                          tab.id
                        );
                        setDraggedTabId(tab.id);
                        setDropTargetTabId(null);
                      }}
                      onDragEnd={(event) => {
                        event.stopPropagation();
                        resetDragState();
                      }}
                    />
                  </Tooltip>
                  <Popover
                    title={i18nText('frontstage', 'design.tab_settings')}
                    content={
                      configuringTab?.id === tab.id ? tabSettingsContent : null
                    }
                    trigger="click"
                    placement="bottomLeft"
                    open={configuringTab?.id === tab.id}
                    destroyOnHidden
                    onOpenChange={(open) => {
                      if (open) {
                        openConfigurePopover(tab);
                      } else if (configuringTab?.id === tab.id) {
                        closeConfigurePopover();
                      }
                    }}
                  >
                    <Tooltip
                      title={i18nText('frontstage', 'design.configure_tab')}
                    >
                      <FrontstageNodeActionButton
                        aria-label={i18nText(
                          'frontstage',
                          'design.configure_tab'
                        )}
                        disabled={!csrfToken}
                        icon={<MenuOutlined />}
                        onClick={(event) => event.stopPropagation()}
                      />
                    </Tooltip>
                  </Popover>
                </span>
              </span>
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
                  style={{
                    borderColor: FRONTSTAGE_DESIGN_BLUE.dashed,
                    color: FRONTSTAGE_DESIGN_BLUE.primary
                  }}
                  onClick={() =>
                    void runMutation(() => createMutation.mutateAsync())
                  }
                >
                  {i18nText('frontstage', 'auto.create_page_tab')}
                </Button>
              </Tooltip>
            ) : undefined
          }
        />
      </ConfigProvider>
      {error ? (
        <Typography.Text className="frontstage-page-tabs__error" type="danger">
          {error}
        </Typography.Text>
      ) : null}
      <div
        className={
          isDesignMode
            ? 'frontstage-page-tabs__content frontstage-page-tabs__content--design-selected'
            : 'frontstage-page-tabs__content'
        }
        data-testid="frontstage-tab-content"
        data-design-selected={isDesignMode ? 'true' : 'false'}
        style={
          isDesignMode
            ? ({
                '--frontstage-design-tab-border':
                  FRONTSTAGE_DESIGN_BLUE.borderSelected,
                '--frontstage-design-tab-halo': FRONTSTAGE_DESIGN_BLUE.halo
              } as CSSProperties)
            : undefined
        }
      >
        {children}
      </div>
    </div>
  );
}
