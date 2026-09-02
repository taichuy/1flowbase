import { Suspense, lazy, useMemo } from 'react';

import { type SettingsExtensionApplicationAction } from '../../api/extensions';
import type { McpBundleImportSource } from '../mcp-management/bundle/McpBundleImportFlow';
import type { I18nCatalogActivationSource } from '../i18n-catalog/I18nCatalogActivationFlow';

const InstalledAgentFlowImportFlow = lazy(() =>
  import('../../../applications/components/InstalledAgentFlowImportFlow').then(
    (module) => ({ default: module.InstalledAgentFlowImportFlow })
  )
);
const McpBundleImportFlow = lazy(() =>
  import('../mcp-management/bundle/McpBundleImportFlow').then((module) => ({
    default: module.McpBundleImportFlow
  }))
);
const I18nCatalogActivationFlow = lazy(() =>
  import('../i18n-catalog/I18nCatalogActivationFlow').then((module) => ({
    default: module.I18nCatalogActivationFlow
  }))
);

export interface ExtensionApplicationTarget {
  installationId?: string;
  builtinTemplateId?: string;
  instanceId?: string;
  action: SettingsExtensionApplicationAction;
}

export function ExtensionApplicationFlow({
  target,
  csrfToken,
  onClose,
  onApplied
}: {
  target: ExtensionApplicationTarget | null;
  csrfToken: string;
  onClose: () => void;
  onApplied: () => Promise<void>;
}) {
  const mcpSource = useMemo<McpBundleImportSource | null>(
    () =>
      target?.action === 'import_mcp'
        ? target.builtinTemplateId && target.instanceId
          ? {
              kind: 'builtin_template',
              templateId: target.builtinTemplateId,
              instanceId: target.instanceId
            }
          : target.installationId
            ? {
                kind: 'installed_extension',
                installationId: target.installationId,
                instanceId: target.instanceId
              }
            : null
        : null,
    [
      target?.action,
      target?.builtinTemplateId,
      target?.installationId,
      target?.instanceId
    ]
  );
  const i18nSource = useMemo<I18nCatalogActivationSource | null>(
    () =>
      target?.action === 'activate_i18n' && target.installationId
        ? {
            kind: 'installed_extension',
            installationId: target.installationId
          }
        : null,
    [target?.action, target?.installationId]
  );

  return (
    <Suspense fallback={null}>
      {target?.action === 'import_agent_flow' ? (
        <InstalledAgentFlowImportFlow
          installationId={target.installationId ?? null}
          csrfToken={csrfToken}
          onClose={onClose}
          onImported={async (applicationId) => {
            await onApplied();
            window.location.assign(
              `/applications/${applicationId}/orchestration`
            );
          }}
        />
      ) : null}
      {mcpSource ? (
        <McpBundleImportFlow
          source={mcpSource}
          csrfToken={csrfToken}
          onClose={onClose}
          onApplied={onApplied}
        />
      ) : null}
      {i18nSource ? (
        <I18nCatalogActivationFlow
          source={i18nSource}
          csrfToken={csrfToken}
          onClose={onClose}
          onActivated={onApplied}
        />
      ) : null}
    </Suspense>
  );
}
