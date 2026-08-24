import {
  sha256Text,
  validateIsolatedFrontendBlockSource
} from '@1flowbase/page-runtime';
import type { IsolatedFrontendBlockProgram } from '@1flowbase/page-protocol';

import type { FrontstageBlockCatalogEntry } from '../api/block-catalog';

export const ISOLATED_FRONTEND_UI_MOUNT_PERMISSION =
  'frontend-block.ui-mount.isolated-realm';

export interface FrontstageIsolatedContributionExpectation {
  blockInstanceId: string;
  workspaceId: string;
  installationId: string;
  providerCode: string;
  pluginId: string;
  pluginVersion: string;
  contributionCode: string;
  props: Record<string, unknown>;
}

export interface PreparedFrontstageIsolatedContribution {
  readonly state: 'prepared';
  readonly blockInstanceId: string;
  readonly contributionId: string;
  readonly blockId: string;
  readonly blockVersion: string;
  readonly graphFingerprint: string;
  readonly runtimeKind: 'isolated';
  readonly executionKind: 'ui_mount';
  readonly isolationRequirement: 'independent_realm';
  readonly lifecycleKind: 'workspace_assignment';
  readonly grantedPermissions: readonly string[];
  readonly assetIntegrity: 'verified_sha256';
  readonly program: IsolatedFrontendBlockProgram;
}

export async function prepareFrontstageIsolatedContribution(
  entry: FrontstageBlockCatalogEntry,
  expected: FrontstageIsolatedContributionExpectation,
  fetchAsset: typeof fetch = globalThis.fetch
): Promise<PreparedFrontstageIsolatedContribution> {
  rejectUnless(
    entry.installation_id === expected.installationId &&
      entry.provider_code === expected.providerCode &&
      entry.plugin_id === expected.pluginId &&
      entry.plugin_version === expected.pluginVersion &&
      entry.contribution_code === expected.contributionCode,
    'catalog identity mismatch'
  );
  rejectUnless(
    entry.workspace_id === expected.workspaceId &&
      entry.frontend_block_version === entry.plugin_version,
    'workspace or version mismatch'
  );
  rejectUnless(
    entry.runtime === 'isolated_iframe' &&
      entry.runtime_kind === 'isolated' &&
      entry.execution_kind === 'ui_mount' &&
      entry.isolation_requirement === 'independent_realm' &&
      entry.lifecycle_kind === 'workspace_assignment',
    'runtime contract mismatch'
  );
  rejectUnless(entry.disable_reason === null, 'contribution is disabled');
  rejectUnless(
    entry.requested_permissions.includes(
      ISOLATED_FRONTEND_UI_MOUNT_PERMISSION
    ) &&
      entry.requested_permissions.every((permission) =>
        entry.granted_permissions.includes(permission)
      ),
    'permission grant mismatch'
  );
  rejectUnless(
    Boolean(
      entry.frontend_contribution_id.trim() &&
      entry.frontend_block_id.trim() &&
      entry.graph_fingerprint.trim()
    ) && entry.provenance.module_kind === 'boot_core',
    'graph provenance mismatch'
  );

  const asset = entry.isolated_entry_asset;
  rejectUnless(
    asset !== undefined &&
      asset !== null &&
      asset.integrity === 'verified_sha256' &&
      asset.media_type.startsWith('text/javascript') &&
      asset.url.endsWith(`/${asset.sha256}`),
    'asset integrity mismatch'
  );

  const response = await fetchAsset(asset.url, {
    credentials: 'same-origin',
    headers: { Accept: asset.media_type }
  });
  rejectUnless(response.ok, 'entry asset unavailable');
  const source = await response.text();
  rejectUnless(sha256Text(source) === asset.sha256, 'asset digest mismatch');
  validateIsolatedFrontendBlockSource(source);

  return Object.freeze({
    state: 'prepared' as const,
    blockInstanceId: expected.blockInstanceId,
    contributionId: entry.frontend_contribution_id,
    blockId: entry.frontend_block_id,
    blockVersion: entry.frontend_block_version,
    graphFingerprint: entry.graph_fingerprint,
    runtimeKind: 'isolated' as const,
    executionKind: 'ui_mount' as const,
    isolationRequirement: 'independent_realm' as const,
    lifecycleKind: 'workspace_assignment' as const,
    grantedPermissions: Object.freeze([...entry.granted_permissions]),
    assetIntegrity: 'verified_sha256' as const,
    program: Object.freeze({ source, props: { ...expected.props } })
  });
}

function rejectUnless(condition: boolean, reason: string): asserts condition {
  if (!condition) {
    throw new Error(`Isolated frontend contribution rejected: ${reason}.`);
  }
}
