import {
  Alert,
  Descriptions,
  List,
  Modal,
  Space,
  Spin,
  Typography,
  message
} from 'antd';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import {
  activateSettingsI18nCatalogUpdate,
  activateSettingsInstalledI18nCatalog,
  fetchSettingsI18nCatalogState,
  fetchSettingsI18nCatalogUpdateStatus,
  previewSettingsInstalledI18nCatalog
} from '../../api/i18n-catalog';

export type I18nCatalogActivationSource =
  | { kind: 'official' }
  | { kind: 'installed_extension'; installationId: string };

type I18nCatalogActivationReview =
  | {
      kind: 'official';
      currentVersion: string | null;
      candidateVersion: string;
      revision: number;
      updateStatus: 'current' | 'update_available';
      integrityWarnings: Array<{ code: string; message: string }>;
    }
  | {
      kind: 'installed_extension';
      installationId: string;
      currentVersion: string | null;
      candidateVersion: string;
      revision: number;
      integrityWarnings: Array<{ code: string; message: string }>;
      integrityOverride?: {
        reason: string;
        acknowledged_warnings: string[];
      };
    };

function confirmedWarnings(warnings: Array<{ code: string }>) {
  return {
    reason: 'user_confirmed',
    acknowledged_warnings: warnings.map((warning) => warning.code)
  };
}

export function I18nCatalogActivationFlow({
  source,
  csrfToken,
  onClose,
  onActivated
}: {
  source: I18nCatalogActivationSource | null;
  csrfToken: string;
  onClose: () => void;
  onActivated: () => Promise<void>;
}) {
  const { t } = useTranslation('settings');
  const [review, setReview] = useState<I18nCatalogActivationReview | null>(
    null
  );
  const [busy, setBusy] = useState(false);
  const sourceKind = source?.kind;
  const installationId =
    source?.kind === 'installed_extension' ? source.installationId : null;

  useEffect(() => {
    setReview(null);
    if (!sourceKind) {
      setBusy(false);
      return;
    }

    let cancelled = false;
    setBusy(true);
    const loadReview = async () => {
      if (sourceKind === 'official') {
        const [state, update] = await Promise.all([
          fetchSettingsI18nCatalogState(),
          fetchSettingsI18nCatalogUpdateStatus()
        ]);
        if (!cancelled) {
          setReview({
            kind: 'official',
            currentVersion: state.active_catalog_version,
            candidateVersion: update.latest_catalog_version,
            revision: state.revision,
            updateStatus: update.status,
            integrityWarnings: []
          });
        }
        return;
      }

      const preview = await previewSettingsInstalledI18nCatalog(
        installationId!
      );
      if (!cancelled) {
        setReview({
          kind: 'installed_extension',
          installationId: installationId!,
          currentVersion: preview.active_catalog_version,
          candidateVersion: preview.installed_catalog_version,
          revision: preview.revision,
          integrityWarnings: preview.integrity_warnings,
          ...(preview.required_integrity_override
            ? {
                integrityOverride: confirmedWarnings(
                  preview.required_integrity_override.warnings
                )
              }
            : {})
        });
      }
    };

    void loadReview()
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
  }, [installationId, onClose, sourceKind]);

  async function activateCatalog() {
    if (!review || !csrfToken) return;
    setBusy(true);
    try {
      if (review.kind === 'official') {
        await activateSettingsI18nCatalogUpdate(
          { expected_revision: review.revision },
          csrfToken
        );
      } else {
        await activateSettingsInstalledI18nCatalog(
          review.installationId,
          {
            expected_revision: review.revision,
            ...(review.integrityOverride
              ? { integrity_override: review.integrityOverride }
              : {})
          },
          csrfToken
        );
      }
      await onActivated();
      message.success(t('auto.translation_catalog_activated'));
      onClose();
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      open={source !== null}
      title={t('auto.activate_translation_catalog')}
      okText={t('auto.activate')}
      confirmLoading={busy}
      okButtonProps={{ disabled: review === null || !csrfToken }}
      onCancel={onClose}
      onOk={() => void activateCatalog()}
    >
      {review ? (
        <Space direction="vertical" size="middle" style={{ width: '100%' }}>
          {review.integrityWarnings.length > 0 ? (
            <List
              size="small"
              dataSource={review.integrityWarnings}
              renderItem={(warning) => <List.Item>{warning.message}</List.Item>}
            />
          ) : null}
          {review.kind === 'official' && review.updateStatus === 'current' ? (
            <Alert
              showIcon
              type="success"
              message={t('auto.translation_catalog_is_current')}
            />
          ) : null}
          <Descriptions size="small" column={1} bordered>
            <Descriptions.Item label={t('auto.active_version')}>
              {review.currentVersion ?? '—'}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.catalog_candidate_version')}>
              {review.candidateVersion}
            </Descriptions.Item>
          </Descriptions>
          <Typography.Text type="secondary">
            {t('auto.translation_catalog_activation_preserves_customizations')}
          </Typography.Text>
        </Space>
      ) : (
        <Spin />
      )}
    </Modal>
  );
}
