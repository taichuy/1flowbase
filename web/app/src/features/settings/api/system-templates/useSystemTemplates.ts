import {
  exportSystemTemplate,
  getSystemTemplateCatalog,
  installSystemTemplate,
  previewSystemTemplate,
  type PortableTemplatePackage,
  type PortableTemplateSelection
} from '@1flowbase/api-client';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useAuthStore } from '../../../../state/auth-store';

const queryKey = ['settings', 'system-templates', 'catalog'] as const;
export function useSystemTemplates(exportOpen: boolean) {
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const catalog = useQuery({
    queryKey,
    queryFn: () => getSystemTemplateCatalog(),
    enabled: exportOpen
  });
  const exportMutation = useMutation({
    mutationFn: (selection: PortableTemplateSelection) =>
      exportSystemTemplate(selection, csrfToken)
  });
  const previewMutation = useMutation({
    mutationFn: (body: PortableTemplatePackage) =>
      previewSystemTemplate(body, csrfToken)
  });
  const installMutation = useMutation({
    mutationFn: (body: PortableTemplatePackage) =>
      installSystemTemplate(body, csrfToken),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey });
    }
  });
  return { catalog, exportMutation, previewMutation, installMutation };
}
