import { useMemo } from 'react';

import { InstalledAgentFlowImportFlow } from '../../../applications/components/InstalledAgentFlowImportFlow';
import { type SettingsExtensionApplicationAction } from '../../api/extensions';
import {
  McpBundleImportFlow,
  type McpBundleImportSource
} from '../mcp-management/bundle/McpBundleImportFlow';
import {
  I18nCatalogActivationFlow,
  type I18nCatalogActivationSource
} from '../i18n-catalog/I18nCatalogActivationFlow';

export interface ExtensionApplicationTarget {
  installationId: string;
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
        ? {
            kind: 'installed_extension',
            installationId: target.installationId
          }
        : null,
    [target?.action, target?.installationId]
  );
  const i18nSource = useMemo<I18nCatalogActivationSource | null>(
    () =>
      target?.action === 'activate_i18n'
        ? {
            kind: 'installed_extension',
            installationId: target.installationId
          }
        : null,
    [target?.action, target?.installationId]
  );

  return (
    <>
      <InstalledAgentFlowImportFlow
        installationId={
          target?.action === 'import_agent_flow' ? target.installationId : null
        }
        csrfToken={csrfToken}
        onClose={onClose}
        onImported={async (applicationId) => {
          await onApplied();
          window.location.assign(
            `/applications/${applicationId}/orchestration`
          );
        }}
      />
      <McpBundleImportFlow
        source={mcpSource}
        csrfToken={csrfToken}
        onClose={onClose}
        onApplied={onApplied}
      />
      <I18nCatalogActivationFlow
        source={i18nSource}
        csrfToken={csrfToken}
        onClose={onClose}
        onActivated={onApplied}
      />
    </>
  );
}
