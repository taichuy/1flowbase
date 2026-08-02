import { useEffect, useMemo, useState } from 'react';

import { message } from 'antd';

import {
  importInstalledApplicationExtension,
  previewInstalledApplicationExtension,
  type AgentFlowTemplatePreview
} from '../../../applications/api/applications';
import { ApplicationTemplateImportModal } from '../../../applications/components/ApplicationTemplateImportModal';
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

function confirmedWarnings(warnings: Array<{ code: string }>) {
  return {
    reason: 'user_confirmed',
    acknowledged_warnings: warnings.map((warning) => warning.code)
  };
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
  const [agentPreview, setAgentPreview] =
    useState<AgentFlowTemplatePreview | null>(null);
  const [agentName, setAgentName] = useState('');
  const [agentWarnings, setAgentWarnings] = useState<string[]>([]);
  const [agentOverride, setAgentOverride] = useState<
    ReturnType<typeof confirmedWarnings> | undefined
  >();
  const [busy, setBusy] = useState(false);
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

  useEffect(() => {
    setAgentPreview(null);
    setAgentWarnings([]);
    setAgentOverride(undefined);
    if (!target || target.action !== 'import_agent_flow') return;

    let cancelled = false;
    setBusy(true);
    const preview = async () => {
      const result = await previewInstalledApplicationExtension(
        target.installationId
      );
      if (cancelled) return;
      setAgentPreview(result.preview);
      setAgentName(result.preview.application.name);
      setAgentWarnings(result.integrity_warnings.map((item) => item.message));
      setAgentOverride(
        result.required_integrity_override
          ? confirmedWarnings(result.required_integrity_override.warnings)
          : undefined
      );
    };
    void preview()
      .catch((error) => {
        if (!cancelled) {
          message.error(error instanceof Error ? error.message : String(error));
          onClose();
        }
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [csrfToken, onClose, target]);

  async function importAgentFlow() {
    if (!target || !agentPreview) return;
    setBusy(true);
    try {
      const imported = await importInstalledApplicationExtension(
        target.installationId,
        {
          name: agentName.trim(),
          description: agentPreview.application.description,
          ...(agentOverride ? { integrity_override: agentOverride } : {})
        },
        csrfToken
      );
      await onApplied();
      window.location.assign(
        `/applications/${imported.application.id}/orchestration`
      );
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <ApplicationTemplateImportModal
        open={target?.action === 'import_agent_flow' && agentPreview !== null}
        preview={agentPreview}
        name={agentName}
        importing={busy}
        integrityWarnings={agentWarnings}
        onNameChange={setAgentName}
        onCancel={onClose}
        onImport={() => void importAgentFlow()}
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
