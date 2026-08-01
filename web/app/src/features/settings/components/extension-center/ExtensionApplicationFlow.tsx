import { useEffect, useState } from 'react';

import { Descriptions, List, Modal, Space, Typography, message } from 'antd';
import { useTranslation } from 'react-i18next';

import {
  importInstalledApplicationExtension,
  previewInstalledApplicationExtension,
  type AgentFlowTemplatePreview
} from '../../../applications/api/applications';
import { ApplicationTemplateImportModal } from '../../../applications/components/ApplicationTemplateImportModal';
import {
  activateSettingsInstalledI18nExtension,
  applySettingsInstalledMcpExtension,
  getSettingsInstalledMcpExtensionConflict,
  getSettingsInstalledMcpExtensionIntegrityChallenge,
  previewSettingsInstalledI18nExtension,
  previewSettingsInstalledMcpExtension,
  type SettingsExtensionApplicationAction
} from '../../api/extensions';
import type { SettingsMcpBundleImportReport } from '../../api/mcp-management';
import {
  McpBundleReviewModal,
  type McpBundleReview
} from '../mcp-management/bundle/McpBundleReviewModal';

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
  const { t } = useTranslation('settingsExtensionCenter');
  const [agentPreview, setAgentPreview] =
    useState<AgentFlowTemplatePreview | null>(null);
  const [agentName, setAgentName] = useState('');
  const [agentWarnings, setAgentWarnings] = useState<string[]>([]);
  const [agentOverride, setAgentOverride] = useState<
    ReturnType<typeof confirmedWarnings> | undefined
  >();
  const [mcpReview, setMcpReview] = useState<McpBundleReview | null>(null);
  const [mcpWarnings, setMcpWarnings] = useState<string[]>([]);
  const [mcpOptions, setMcpOptions] = useState<
    Parameters<typeof applySettingsInstalledMcpExtension>[2]
  >({});
  const [i18nPreview, setI18nPreview] = useState<Awaited<
    ReturnType<typeof previewSettingsInstalledI18nExtension>
  > | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setAgentPreview(null);
    setMcpReview(null);
    setI18nPreview(null);
    setAgentWarnings([]);
    setMcpWarnings([]);
    setAgentOverride(undefined);
    setMcpOptions({});
    if (!target) return;

    let cancelled = false;
    setBusy(true);
    const preview = async () => {
      if (target.action === 'import_agent_flow') {
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
      } else if (target.action === 'import_mcp') {
        const result = await previewSettingsInstalledMcpExtension(
          target.installationId,
          csrfToken
        );
        if (cancelled) return;
        setMcpReview(result.preview);
        setMcpWarnings(result.integrity_warnings.map((item) => item.message));
        setMcpOptions({
          ...(result.required_conflict_resolution
            ? { conflict_resolution: result.required_conflict_resolution }
            : {}),
          ...(result.required_integrity_override
            ? {
                integrity_override: confirmedWarnings(
                  result.required_integrity_override.warnings
                )
              }
            : {})
        });
      } else if (target.action === 'activate_i18n') {
        const result = await previewSettingsInstalledI18nExtension(
          target.installationId
        );
        if (!cancelled) setI18nPreview(result);
      }
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

  async function importMcp() {
    if (!target) return;
    setBusy(true);
    try {
      const result = await applySettingsInstalledMcpExtension(
        target.installationId,
        csrfToken,
        mcpOptions
      );
      setMcpReview(result.import_report as SettingsMcpBundleImportReport);
      await onApplied();
    } catch (error) {
      const conflict = getSettingsInstalledMcpExtensionConflict(error);
      const integrity =
        getSettingsInstalledMcpExtensionIntegrityChallenge(error);
      const challenge = conflict ?? integrity;
      if (!challenge) {
        message.error(error instanceof Error ? error.message : String(error));
      } else {
        setMcpReview(challenge.preview);
        setMcpWarnings(
          challenge.integrity_warnings.map((item) => item.message)
        );
        setMcpOptions({
          ...mcpOptions,
          ...('required_conflict_resolution' in challenge
            ? { conflict_resolution: challenge.required_conflict_resolution }
            : {}),
          ...('required_integrity_override' in challenge
            ? {
                integrity_override: confirmedWarnings(
                  challenge.required_integrity_override.warnings
                )
              }
            : {})
        });
      }
    } finally {
      setBusy(false);
    }
  }

  async function activateI18n() {
    if (!target || !i18nPreview) return;
    setBusy(true);
    try {
      await activateSettingsInstalledI18nExtension(
        target.installationId,
        {
          expected_revision: i18nPreview.revision,
          ...(i18nPreview.required_integrity_override
            ? {
                integrity_override: confirmedWarnings(
                  i18nPreview.required_integrity_override.warnings
                )
              }
            : {})
        },
        csrfToken
      );
      message.success(t('auto.translation_catalog_activated'));
      await onApplied();
      onClose();
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
      <McpBundleReviewModal
        review={target?.action === 'import_mcp' ? mcpReview : null}
        importing={busy}
        integrityWarnings={mcpWarnings}
        onCancel={onClose}
        onImport={() => void importMcp()}
      />
      <Modal
        open={target?.action === 'activate_i18n' && i18nPreview !== null}
        title={t('auto.activate_translation_catalog')}
        okText={t('auto.activate')}
        confirmLoading={busy}
        onCancel={onClose}
        onOk={() => void activateI18n()}
      >
        {i18nPreview ? (
          <Space direction="vertical" size="middle" style={{ width: '100%' }}>
            {i18nPreview.integrity_warnings.length > 0 ? (
              <List
                size="small"
                dataSource={i18nPreview.integrity_warnings}
                renderItem={(warning) => (
                  <List.Item>{warning.message}</List.Item>
                )}
              />
            ) : null}
            <Descriptions size="small" column={1} bordered>
              <Descriptions.Item label={t('auto.active_version')}>
                {i18nPreview.active_catalog_version ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.installed_version')}>
                {i18nPreview.installed_catalog_version}
              </Descriptions.Item>
            </Descriptions>
            <Typography.Text type="secondary">
              {t(
                'auto.translation_catalog_activation_preserves_customizations'
              )}
            </Typography.Text>
          </Space>
        ) : null}
      </Modal>
    </>
  );
}
