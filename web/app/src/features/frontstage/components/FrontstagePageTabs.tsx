import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Button, Input, Space, Tabs, Typography } from 'antd';
import { useMemo, useState } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { i18nText } from '../../../shared/i18n/text';
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

export function FrontstagePageTabs({
  workspaceId,
  pageId,
  tabId,
  isDesignMode,
  onNavigateTab
}: FrontstagePageTabsProps) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [draftTitle, setDraftTitle] = useState('');
  const [error, setError] = useState<string | null>(null);
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
        { title: draftTitle.trim() || null, rank: nextRank(tabs) },
        csrfToken ?? ''
      ),
    onSuccess: (createdTab) => {
      setDraftTitle('');
      onNavigateTab(createdTab.id);
    }
  });

  if (!isDesignMode && tabs.length <= 1) {
    return null;
  }

  const currentIndex = tabs.findIndex((tab) => tab.id === tabId);
  const currentTab = currentIndex >= 0 ? tabs[currentIndex] : null;

  return (
    <Space direction="vertical" size={8} style={{ width: '100%' }}>
      <Tabs
        activeKey={tabId}
        onChange={onNavigateTab}
        items={tabs.map((tab) => ({
          key: tab.id,
          label:
            tab.title?.trim() ||
            i18nText('frontstage', 'auto.unnamed_page_tab')
        }))}
      />
      {isDesignMode ? (
        <Space wrap>
          <Input
            aria-label={i18nText('frontstage', 'auto.page_tab_name')}
            value={draftTitle}
            onChange={(event) => setDraftTitle(event.target.value)}
            placeholder={i18nText('frontstage', 'auto.page_tab_name')}
          />
          <Button
            onClick={() => void runMutation(() => createMutation.mutateAsync())}
            disabled={!csrfToken}
          >
            {i18nText('frontstage', 'auto.create_page_tab')}
          </Button>
          <Button
            disabled={!currentTab || !csrfToken}
            onClick={() => {
              if (!currentTab) return;
              const title = draftTitle.trim() || currentTab.title;
              void runMutation(() =>
                renameFrontstagePageTab(
                  workspaceId,
                  pageId,
                  currentTab.id,
                  { title },
                  csrfToken ?? ''
                )
              );
            }}
          >
            {i18nText('frontstage', 'auto.rename_current_page_tab')}
          </Button>
          <Button
            disabled={currentIndex <= 0 || !csrfToken}
            onClick={() => {
              if (!currentTab || currentIndex <= 0) return;
              void runMutation(() =>
                moveFrontstagePageTab(
                  workspaceId,
                  pageId,
                  currentTab.id,
                  { rank: previousRank(currentIndex) },
                  csrfToken ?? ''
                )
              );
            }}
          >
            {i18nText('frontstage', 'auto.move_current_page_tab_forward')}
          </Button>
          <Button
            danger
            disabled={!currentTab || !csrfToken}
            onClick={() => {
              if (!currentTab) return;
              const nextTab = tabs[currentIndex + 1] ?? tabs[currentIndex - 1];
              void runMutation(() =>
                deleteFrontstagePageTab(
                  workspaceId,
                  pageId,
                  currentTab.id,
                  csrfToken ?? ''
                )
              ).then((deleted) => {
                if (deleted !== null && nextTab) {
                  onNavigateTab(nextTab.id);
                }
              });
            }}
          >
            {i18nText('frontstage', 'auto.delete_current_page_tab')}
          </Button>
        </Space>
      ) : null}
      {error ? <Typography.Text type="danger">{error}</Typography.Text> : null}
    </Space>
  );
}
