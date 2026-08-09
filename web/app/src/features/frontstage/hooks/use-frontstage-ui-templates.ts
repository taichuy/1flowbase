import { listFrontstageUiTemplates } from '@1flowbase/api-client';
import { useQuery } from '@tanstack/react-query';

export function useFrontstageUiTemplates(workspaceId: string, enabled = true) {
  return useQuery({
    queryKey: ['frontstage', workspaceId, 'ui-templates'],
    queryFn: () => listFrontstageUiTemplates(workspaceId),
    enabled
  });
}
