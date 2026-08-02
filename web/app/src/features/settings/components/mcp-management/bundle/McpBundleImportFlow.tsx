import { message } from 'antd';
import { useEffect, useState } from 'react';

import {
  applySettingsInstalledMcpExtension,
  getSettingsInstalledMcpExtensionConflict,
  getSettingsInstalledMcpExtensionIntegrityChallenge,
  previewSettingsInstalledMcpExtension
} from '../../../api/extensions';
import {
  importSettingsMcpBundle,
  importSettingsOfficialMcpBundle,
  previewSettingsMcpBundle,
  previewSettingsOfficialMcpBundle
} from '../../../api/mcp-management';
import {
  McpBundleReviewModal,
  type McpBundleReview
} from './McpBundleReviewModal';

export type McpBundleImportSource =
  | { kind: 'upload'; file: File }
  | { kind: 'official'; organization: string; bundleId: string }
  | { kind: 'installed_extension'; installationId: string };

function confirmedWarnings(warnings: Array<{ code: string }>) {
  return {
    reason: 'user_confirmed',
    acknowledged_warnings: warnings.map((warning) => warning.code)
  };
}

export function McpBundleImportFlow({
  source,
  csrfToken,
  onClose,
  onApplied
}: {
  source: McpBundleImportSource | null;
  csrfToken: string;
  onClose: () => void;
  onApplied: () => Promise<void>;
}) {
  const [review, setReview] = useState<McpBundleReview | null>(null);
  const [integrityWarnings, setIntegrityWarnings] = useState<string[]>([]);
  const [installedOptions, setInstalledOptions] = useState<
    NonNullable<Parameters<typeof applySettingsInstalledMcpExtension>[2]>
  >({});
  const [busy, setBusy] = useState(false);
  const sourceKind = source?.kind;
  const uploadFile = source?.kind === 'upload' ? source.file : null;
  const officialOrganization =
    source?.kind === 'official' ? source.organization : null;
  const officialBundleId = source?.kind === 'official' ? source.bundleId : null;
  const installationId =
    source?.kind === 'installed_extension' ? source.installationId : null;

  useEffect(() => {
    setReview(null);
    setIntegrityWarnings([]);
    setInstalledOptions({});
    if (!sourceKind) {
      setBusy(false);
      return;
    }

    let cancelled = false;
    setBusy(true);
    const preview = async () => {
      if (sourceKind === 'upload') {
        return previewSettingsMcpBundle(uploadFile!, csrfToken);
      }
      if (sourceKind === 'official') {
        return previewSettingsOfficialMcpBundle(
          {
            organization: officialOrganization!,
            bundle_id: officialBundleId!
          },
          csrfToken
        );
      }

      const result = await previewSettingsInstalledMcpExtension(
        installationId!,
        csrfToken
      );
      if (!cancelled) {
        setIntegrityWarnings(
          result.integrity_warnings.map((warning) => warning.message)
        );
        setInstalledOptions({
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
      }
      return result.preview;
    };

    void preview()
      .then((result) => {
        if (!cancelled) setReview(result);
      })
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
  }, [
    csrfToken,
    installationId,
    officialBundleId,
    officialOrganization,
    onClose,
    sourceKind,
    uploadFile
  ]);

  async function importBundle() {
    if (!source || !review || !csrfToken) return;
    if (
      review.effect_summary.changes === 0 &&
      review.effect_summary.conflicts > 0
    ) {
      return;
    }

    setBusy(true);
    try {
      const report =
        source.kind === 'upload'
          ? await importSettingsMcpBundle(source.file, csrfToken)
          : source.kind === 'official'
            ? await importSettingsOfficialMcpBundle(
                {
                  organization: source.organization,
                  bundle_id: source.bundleId
                },
                csrfToken
              )
            : (
                await applySettingsInstalledMcpExtension(
                  source.installationId,
                  csrfToken,
                  installedOptions
                )
              ).import_report;
      setReview(report);
      await onApplied();
    } catch (error) {
      if (source.kind !== 'installed_extension') {
        message.error(error instanceof Error ? error.message : String(error));
        return;
      }
      const challenge =
        getSettingsInstalledMcpExtensionConflict(error) ??
        getSettingsInstalledMcpExtensionIntegrityChallenge(error);
      if (!challenge) {
        message.error(error instanceof Error ? error.message : String(error));
        return;
      }
      setReview(challenge.preview);
      setIntegrityWarnings(
        challenge.integrity_warnings.map((warning) => warning.message)
      );
      setInstalledOptions((current) => ({
        ...current,
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
      }));
    } finally {
      setBusy(false);
    }
  }

  return (
    <McpBundleReviewModal
      open={source !== null}
      review={review}
      loading={busy && review === null}
      importing={busy && review !== null}
      integrityWarnings={integrityWarnings}
      onCancel={onClose}
      onImport={() => void importBundle()}
    />
  );
}
