import type { FrontstageBlockCatalogEntry } from '../api/block-catalog';

export const TRUSTED_FRONTEND_UI_MOUNT_PERMISSION =
  'frontend-block.ui-mount.trusted-host';

export type TrustedFrontendContributionLifecycleState =
  | 'discovered'
  | 'prepared'
  | 'mounted'
  | 'updated'
  | 'failed'
  | 'disposed';

export type TrustedFrontendContributionHandleState = Exclude<
  TrustedFrontendContributionLifecycleState,
  'discovered'
>;

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
  readonly state: 'prepared';
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

/** Catalog selection is observable as discovery; trusted gate validation prepares it. */
export interface DiscoveredTrustedFrontendContribution {
  readonly state: 'discovered';
  prepare(): PreparedTrustedFrontendContribution;
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
  private lifecycleState: TrustedFrontendContributionHandleState = 'prepared';
  private mountOwner: TrustedFrontendContributionMountOwner | null = null;

  constructor(readonly contribution: PreparedTrustedFrontendContribution) {}

  get state(): TrustedFrontendContributionHandleState {
    return this.lifecycleState;
  }

  mount(owner: TrustedFrontendContributionMountOwner): void {
    this.requireState('prepared', 'mount');
    this.mountOwner = owner;
    try {
      owner.mount();
      this.lifecycleState = 'mounted';
    } catch (error) {
      this.lifecycleState = 'failed';
      this.mountOwner = null;
      try {
        owner.dispose();
      } catch {
        // The original mount error defines the failed lifecycle receipt.
      }
      throw error;
    }
  }

  update(): void {
    if (
      this.lifecycleState !== 'mounted' &&
      this.lifecycleState !== 'updated'
    ) {
      this.invalidTransition('update');
    }
    this.lifecycleState = 'updated';
  }

  dispose(): void {
    if (
      this.lifecycleState === 'disposed' ||
      this.lifecycleState === 'failed'
    ) {
      return;
    }
    if (this.lifecycleState === 'prepared') {
      this.lifecycleState = 'disposed';
      return;
    }
    const owner = this.mountOwner;
    this.mountOwner = null;
    this.lifecycleState = 'disposed';
    owner?.dispose();
  }

  private requireState(
    expected: TrustedFrontendContributionHandleState,
    operation: 'mount' | 'update' | 'dispose'
  ): void {
    if (this.lifecycleState !== expected) {
      this.invalidTransition(operation);
    }
  }

  private invalidTransition(operation: 'mount' | 'update' | 'dispose'): never {
    throw new TrustedFrontendContributionLifecycleError(
      'invalid_transition',
      `Cannot ${operation} trusted frontend contribution ${this.contribution.contributionId} while lifecycle is ${this.lifecycleState}.`
    );
  }
}

export function discoverTrustedFrontendContribution(
  entry: FrontstageBlockCatalogEntry,
  expected: TrustedFrontendContributionExpectation
): DiscoveredTrustedFrontendContribution {
  return Object.freeze({
    state: 'discovered' as const,
    prepare: () => prepareDiscoveredTrustedFrontendContribution(entry, expected)
  });
}

export function prepareTrustedFrontendContribution(
  entry: FrontstageBlockCatalogEntry,
  expected: TrustedFrontendContributionExpectation
): PreparedTrustedFrontendContribution {
  return discoverTrustedFrontendContribution(entry, expected).prepare();
}

function prepareDiscoveredTrustedFrontendContribution(
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

  const contribution: PreparedTrustedFrontendContribution = Object.freeze({
    state: 'prepared' as const,
    contributionId: entry.frontend_contribution_id,
    blockId: entry.frontend_block_id,
    blockVersion: entry.frontend_block_version,
    assetIntegrity: Object.freeze([]),
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
