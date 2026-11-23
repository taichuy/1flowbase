import { useQuery } from '@tanstack/react-query';
import { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';
import {
  getSeededPublicSystemMetadata,
  getSeededPublicSystemMetadataUpdatedAt
} from './system-metadata-seed';

export const systemMetadataQueryKey = ['public', 'system-metadata'] as const;

type SystemMetadataResponse = Awaited<
  ReturnType<typeof fetchPublicSystemMetadata>
>;

export function useSystemMetadata() {
  const seeded = getSeededPublicSystemMetadata();
  const seededAt = getSeededPublicSystemMetadataUpdatedAt();
  const hasCompleteSeed = Boolean(
    seeded?.system_name !== undefined &&
      seeded?.hide_login_tagline !== undefined &&
      seededAt > 0
  );

  return useQuery({
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
}
