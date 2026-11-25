import { useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';
import {
  getSeededPublicSystemMetadata,
  getSeededPublicSystemMetadataUpdatedAt,
  SYSTEM_METADATA_INVALIDATE_EVENT
} from './system-metadata-seed';

export const systemMetadataQueryKey = ['public', 'system-metadata'] as const;

type SystemMetadataResponse = Awaited<
  ReturnType<typeof fetchPublicSystemMetadata>
>;

export function useSystemMetadata() {
  const queryClient = useQueryClient();
  const seeded = getSeededPublicSystemMetadata();
  const seededAt = getSeededPublicSystemMetadataUpdatedAt();
  const hasCompleteSeed = Boolean(
    seeded?.system_name !== undefined &&
      seeded?.hide_login_tagline !== undefined &&
      seededAt > 0
  );

  const query = useQuery({
    queryKey: systemMetadataQueryKey,
    queryFn: fetchPublicSystemMetadata,
    staleTime: 60_000,
    initialData: hasCompleteSeed ? (seeded as SystemMetadataResponse) : undefined,
    initialDataUpdatedAt: hasCompleteSeed ? seededAt : undefined,
    placeholderData: () => {
      const current = getSeededPublicSystemMetadata();
      return current?.system_name
        ? (current as SystemMetadataResponse)
        : undefined;
    }
  });

  useEffect(() => {
    const nextTitle =
      query.data?.page_title?.trim() || query.data?.system_name?.trim();
    if (nextTitle && document.title !== nextTitle) {
      document.title = nextTitle;
    }
  }, [query.data?.page_title, query.data?.system_name]);

  useEffect(() => {
    const onInvalidate = () => {
      void queryClient.invalidateQueries({ queryKey: systemMetadataQueryKey });
    };
    window.addEventListener(SYSTEM_METADATA_INVALIDATE_EVENT, onInvalidate);
    return () => {
      window.removeEventListener(SYSTEM_METADATA_INVALIDATE_EVENT, onInvalidate);
    };
  }, [queryClient]);

  return query;
}
