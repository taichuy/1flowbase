import { useQuery } from '@tanstack/react-query';
import { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';

export const systemMetadataQueryKey = ['public', 'system-metadata'] as const;

export function useSystemMetadata() {
  return useQuery({
    queryKey: systemMetadataQueryKey,
    queryFn: fetchPublicSystemMetadata,
    staleTime: 60_000
  });
}
