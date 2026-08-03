import { useEffect, useState } from 'react';

import { message } from 'antd';

import {
  importInstalledApplicationExtension,
  previewInstalledApplicationExtension,
  type AgentFlowTemplatePreview
} from '../api/applications';
import { ApplicationTemplateImportModal } from './ApplicationTemplateImportModal';

function confirmedWarnings(warnings: Array<{ code: string }>) {
  return {
    reason: 'user_confirmed',
    acknowledged_warnings: warnings.map((warning) => warning.code)
  };
}

export function InstalledAgentFlowImportFlow({
  installationId,
  csrfToken,
  onClose,
  onImported
}: {
  installationId: string | null;
  csrfToken: string;
  onClose: () => void;
  onImported: (applicationId: string) => Promise<void>;
}) {
  const [preview, setPreview] = useState<AgentFlowTemplatePreview | null>(null);
  const [name, setName] = useState('');
  const [integrityWarnings, setIntegrityWarnings] = useState<string[]>([]);
  const [integrityOverride, setIntegrityOverride] = useState<
    ReturnType<typeof confirmedWarnings> | undefined
  >();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setPreview(null);
    setIntegrityWarnings([]);
    setIntegrityOverride(undefined);
    if (!installationId) return;

    let cancelled = false;
    setBusy(true);
    void previewInstalledApplicationExtension(installationId)
      .then((result) => {
        if (cancelled) return;
        setPreview(result.preview);
        setName(result.preview.application.name);
        setIntegrityWarnings(
          result.integrity_warnings.map((warning) => warning.message)
        );
        setIntegrityOverride(
          result.required_integrity_override
            ? confirmedWarnings(result.required_integrity_override.warnings)
            : undefined
        );
      })
      .catch((error) => {
        if (cancelled) return;
        message.error(error instanceof Error ? error.message : String(error));
        onClose();
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });

    return () => {
      cancelled = true;
    };
  }, [installationId, onClose]);

  async function importAgentFlow() {
    if (!installationId || !preview) return;
    setBusy(true);
    try {
      const result = await importInstalledApplicationExtension(
        installationId,
        {
          name: name.trim(),
          description: preview.application.description,
          ...(integrityOverride
            ? { integrity_override: integrityOverride }
            : {})
        },
        csrfToken
      );
      await onImported(result.application.id);
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <ApplicationTemplateImportModal
      open={Boolean(installationId && preview)}
      preview={preview}
      name={name}
      importing={busy}
      integrityWarnings={integrityWarnings}
      onNameChange={setName}
      onCancel={onClose}
      onImport={() => void importAgentFlow()}
    />
  );
}
