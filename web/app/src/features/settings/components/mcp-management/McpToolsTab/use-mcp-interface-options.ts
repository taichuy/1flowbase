import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { useCallback, useMemo, useState, type UIEvent } from 'react';
import type { ConsoleMcpInterfaceCapabilitySummary } from '@1flowbase/api-client';

import {
  fetchSettingsMcpInterfaceCapabilitiesPage,
  fetchSettingsMcpInterfaceCapability,
  settingsMcpInterfaceCapabilitiesQueryKey,
  settingsMcpInterfaceCapabilityDetailQueryKey
} from '../../../api/mcp-management';
import { interfaceOptionLabel } from './tool-editor-model';

const MCP_INTERFACE_OPTIONS_PAGE_SIZE = 20;

export function useMcpInterfaceOptions({
  selectedInterfaceId,
  includeDisabledSuffix = false
}: {
  selectedInterfaceId?: string;
  includeDisabledSuffix?: boolean;
}) {
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [searchText, setSearchText] = useState('');
  const normalizedQuery = searchText.trim();
  const listEnabled = dropdownOpen || normalizedQuery.length > 0;

  const pagesQuery = useInfiniteQuery({
    queryKey: [
      ...settingsMcpInterfaceCapabilitiesQueryKey,
      'page',
      { q: normalizedQuery, limit: MCP_INTERFACE_OPTIONS_PAGE_SIZE }
    ],
    initialPageParam: 0,
    queryFn: ({ pageParam }) =>
      fetchSettingsMcpInterfaceCapabilitiesPage({
        bindable_only: false,
        q: normalizedQuery || undefined,
        offset: pageParam,
        limit: MCP_INTERFACE_OPTIONS_PAGE_SIZE
      }),
    getNextPageParam: (lastPage) =>
      lastPage.has_more ? (lastPage.next_offset ?? undefined) : undefined,
    enabled: listEnabled
  });

  const loadedItems = useMemo(() => {
    const items = new Map<string, ConsoleMcpInterfaceCapabilitySummary>();
    for (const page of pagesQuery.data?.pages ?? []) {
      for (const entry of page.items) {
        items.set(entry.interface_id, entry);
      }
    }
    return items;
  }, [pagesQuery.data]);

  const missingSelectedId =
    selectedInterfaceId && !loadedItems.has(selectedInterfaceId)
      ? selectedInterfaceId
      : undefined;

  const selectedDetailQuery = useQuery({
    queryKey: settingsMcpInterfaceCapabilityDetailQueryKey(
      missingSelectedId ?? ''
    ),
    queryFn: () => fetchSettingsMcpInterfaceCapability(missingSelectedId!),
    enabled: Boolean(missingSelectedId)
  });

  const formatOption = useCallback(
    (entry: ConsoleMcpInterfaceCapabilitySummary) =>
      includeDisabledSuffix
        ? {
            label: `${interfaceOptionLabel(entry)} - ${entry.interface_id}${
              entry.bindable ? '' : ` (${entry.disabled_reason})`
            }`,
            value: entry.interface_id,
            disabled: !entry.bindable
          }
        : {
            label: `${interfaceOptionLabel(entry)} ${entry.interface_id}`,
            value: entry.interface_id
          },
    [includeDisabledSuffix]
  );

  const options = useMemo(() => {
    const entries = [...loadedItems.values()];
    if (
      selectedInterfaceId &&
      !loadedItems.has(selectedInterfaceId) &&
      selectedDetailQuery.data?.interface_id === selectedInterfaceId
    ) {
      entries.unshift(selectedDetailQuery.data);
    }
    return entries.map(formatOption);
  }, [formatOption, loadedItems, selectedDetailQuery.data, selectedInterfaceId]);

  const handlePopupScroll = useCallback(
    (event: UIEvent<HTMLDivElement>) => {
      const target = event.currentTarget;
      if (
        pagesQuery.hasNextPage &&
        !pagesQuery.isFetchingNextPage &&
        target.scrollTop + target.clientHeight >= target.scrollHeight - 24
      ) {
        void pagesQuery.fetchNextPage();
      }
    },
    [pagesQuery]
  );

  return {
    options,
    loading: pagesQuery.isLoading || pagesQuery.isFetchingNextPage,
    onOpenChange: (open: boolean) => {
      setDropdownOpen(open);
      if (!open) setSearchText('');
    },
    onSearch: setSearchText,
    onPopupScroll: handlePopupScroll
  };
}
