import type { FrontstageBlockCatalogEntry } from '../api/block-catalog';

export const TRUSTED_FRONTEND_UI_MOUNT_PERMISSION =
  'frontend-block.ui-mount.trusted-host';

export type TrustedFrontendContributionLifecycleState =
  | 'prepared'
  | 'mounted'
  | 'disposed';

export type TrustedFrontendContributionLifecycleErrorCode =
  | 'binding_rejected'
  | 'invalid_transition';

export class TrustedFrontendContributionLifecycleError extends Error {
  constructor(
    readonly code: TrustedFrontendContributionLifecycleErrorCode,
    message: string
  ) {
    super(message);
    this.name = 'TrustedFrontendContributionLifecycleError';
  }
}

export interface TrustedFrontendContributionMountOwner {
  mount(): void;
  dispose(): void;
}

export interface PreparedTrustedFrontendContribution {
  readonly contributionId: string;
  readonly blockId: string;
  readonly blockVersion: string;
  readonly assetIntegrity: readonly 'verified_sha256'[];
  readonly grantedPermissions: readonly string[];
  readonly graphFingerprint: string;
  readonly runtimeKind: 'trusted_native';
  readonly executionKind: 'ui_mount';
  readonly isolationRequirement: 'trusted_host_realm';
  readonly lifecycleKind: 'workspace_assignment';
  createHandle(): TrustedFrontendContributionHandle;
}

export interface TrustedFrontendContributionExpectation {
  workspaceId: string;
  installationId: string;
  providerCode: string;
  pluginId: string;
  pluginVersion: string;
  contributionCode: string;
}

/**
 * Tracks one mounted UI instance. Disposal releases instance-owned resources;
 * it does not unload an evaluated JavaScript module or provide a security realm.
 */
export class TrustedFrontendContributionHandle {
  private lifecycleState: TrustedFrontendContributionLifecycleState =
    'prepared';
  private mountOwner: TrustedFrontendContributionMountOwner | null = null;

  constructor(readonly contribution: PreparedTrustedFrontendContribution) {}

  get state(): TrustedFrontendContributionLifecycleState {
    return this.lifecycleState;
  }

  mount(owner: TrustedFrontendContributionMountOwner): void {
    this.requireState('prepared', 'mount');
    this.mountOwner = owner;
    try {
      owner.mount();
      this.lifecycleState = 'mounted';
    } catch (error) {
      this.lifecycleState = 'disposed';
      this.mountOwner = null;
      owner.dispose();
      throw error;
    }
  }

  update(): void {
    this.requireState('mounted', 'update');
  }

  dispose(): void {
    this.requireState('mounted', 'dispose');
    const owner = this.mountOwner;
    this.mountOwner = null;
    this.lifecycleState = 'disposed';
    owner?.dispose();
  }

  private requireState(
    expected: TrustedFrontendContributionLifecycleState,
    operation: 'mount' | 'update' | 'dispose'
  ): void {
    if (this.lifecycleState !== expected) {
      throw new TrustedFrontendContributionLifecycleError(
        'invalid_transition',
        `Cannot ${operation} trusted frontend contribution ${this.contribution.contributionId} while lifecycle is ${this.lifecycleState}.`
      );
    }
  }
}

export function prepareTrustedFrontendContribution(
  entry: FrontstageBlockCatalogEntry,
  expected: TrustedFrontendContributionExpectation
): PreparedTrustedFrontendContribution {
  const reject = (reason: string): never => {
    throw new TrustedFrontendContributionLifecycleError(
      'binding_rejected',
      `Trusted frontend contribution binding was rejected: ${reason}.`
    );
  };

  if (
    entry.installation_id !== expected.installationId ||
    entry.provider_code !== expected.providerCode ||
    entry.plugin_id !== expected.pluginId ||
    entry.plugin_version !== expected.pluginVersion ||
    entry.contribution_code !== expected.contributionCode
  ) {
    reject('catalog identity mismatch');
  }
  if (
    entry.workspace_id !== expected.workspaceId ||
    entry.frontend_block_version !== entry.plugin_version
  ) {
    reject('workspace or version mismatch');
  }
  if (
    entry.runtime_kind !== 'trusted_native' ||
    entry.execution_kind !== 'ui_mount' ||
    entry.isolation_requirement !== 'trusted_host_realm' ||
    entry.lifecycle_kind !== 'workspace_assignment'
  ) {
    reject('runtime contract mismatch');
  }
  if (entry.disable_reason !== null) {
    reject(`contribution disabled by ${entry.disable_reason}`);
  }
  if (
    !entry.requested_permissions.includes(
      TRUSTED_FRONTEND_UI_MOUNT_PERMISSION
    ) ||
    !entry.requested_permissions.every((permission) =>
      entry.granted_permissions.includes(permission)
    )
  ) {
    reject('permission grant mismatch');
  }
  if (
    !entry.frontend_contribution_id.trim() ||
    !entry.frontend_block_id.trim() ||
    !entry.graph_fingerprint.trim() ||
    entry.provenance.module_kind !== 'boot_core'
  ) {
    reject('graph provenance mismatch');
  }

  let projectedAssetCount = 0;
  for (const asset of entry.code_modules.flatMap((module) => module.assets)) {
    if (!('integrity' in asset)) continue;
    projectedAssetCount += 1;
    if (
      asset.integrity !== 'verified_sha256' ||
      !asset.url ||
      !asset.sha256 ||
      !asset.url.endsWith(`/${asset.sha256}`)
    ) {
      reject('asset integrity mismatch');
    }
  }

  const contribution: PreparedTrustedFrontendContribution = Object.freeze({
    contributionId: entry.frontend_contribution_id,
    blockId: entry.frontend_block_id,
    blockVersion: entry.frontend_block_version,
    assetIntegrity: Object.freeze(
      Array.from(
        { length: projectedAssetCount },
        () => 'verified_sha256' as const
      )
    ),
    grantedPermissions: Object.freeze([...entry.granted_permissions]),
    graphFingerprint: entry.graph_fingerprint,
    runtimeKind: 'trusted_native' as const,
    executionKind: 'ui_mount' as const,
    isolationRequirement: 'trusted_host_realm' as const,
    lifecycleKind: 'workspace_assignment' as const,
    createHandle: () => new TrustedFrontendContributionHandle(contribution)
  });
  return contribution;
}
