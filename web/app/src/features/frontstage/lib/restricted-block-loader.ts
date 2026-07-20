import type {
  BlockDataPermission,
  BlockUiSchemaValidationOptions
} from '@1flowbase/page-protocol';
import type {
  BlockContextMediatorPolicy,
  JsBlockRunRequest,
  JsBlockRuntimeLimits
} from '@1flowbase/page-runtime';

import {
  hasFrontstageBlockActionPermission,
  hasFrontstageBlockDataPermission,
  hasFrontstageBlockEventPermission,
  isFrontstageBlockRestrictedRuntime,
  type NormalizedFrontstageBlockCatalogEntry
} from './block-catalog';
import type { FrontstageBlockInstance } from './page-document';

export interface RestrictedBlockLoaderLimits extends JsBlockRuntimeLimits {
  allowedActions?: readonly string[];
  allowedEvents?: readonly string[];
  allowedQueries?: readonly string[];
  allowedDataOperations?: readonly BlockDataPermission[];
  allowedInterfaces?: readonly string[];
  maxEventChainDepth?: number;
}

export type RestrictedBlockLoaderRejectionCode =
  | 'catalog_mismatch'
  | 'unsupported_runtime'
  | 'missing_code_ref'
  | 'missing_code'
  | 'missing_limits'
  | 'invalid_input';

export interface RestrictedBlockLoaderRejection {
  ok: false;
  code: RestrictedBlockLoaderRejectionCode;
  path: string;
  message: string;
  blockId?: string;
  catalogId?: string;
}

export interface RestrictedBlockRunPlan {
  ok: true;
  request: JsBlockRunRequest;
  schemaValidationOptions: BlockUiSchemaValidationOptions;
  mediatorPolicy: BlockContextMediatorPolicy;
}

export type RestrictedBlockLoaderResult =
  | RestrictedBlockRunPlan
  | RestrictedBlockLoaderRejection;

export interface RestrictedBlockLoaderInput {
  block: FrontstageBlockInstance;
  catalogEntry: NormalizedFrontstageBlockCatalogEntry;
  code: string;
  contextSnapshot: Record<string, unknown>;
  props?: Record<string, unknown>;
  state?: Record<string, unknown>;
  inputs?: Record<string, unknown>;
  limits?: RestrictedBlockLoaderLimits;
}

export function createRestrictedBlockRunPlan(
  input: RestrictedBlockLoaderInput
): RestrictedBlockLoaderResult {
  const base = getRejectionBase(input.block, input.catalogEntry);
  const codeRef = normalizeRequiredString(input.block.codeRef);

  if (!codeRef || !normalizeRequiredString(input.block.sourceCodeRef)) {
    return reject(base, {
      code: 'missing_code_ref',
      path: 'block.codeRef',
      message: 'Restricted block codeRef is required.'
    });
  }

  const catalogValidation = validateCatalogMatch(
    input.block,
    input.catalogEntry
  );
  if (catalogValidation) {
    return reject(base, catalogValidation);
  }

  const runtimeValidation = validateRuntime(input.block, input.catalogEntry);
  if (runtimeValidation) {
    return reject(base, runtimeValidation);
  }

  if (typeof input.code !== 'string' || input.code.trim().length === 0) {
    return reject(base, {
      code: 'missing_code',
      path: 'code',
      message: 'Restricted block code is required.'
    });
  }

  const limits = normalizeLimits(input.limits);
  if (!limits.ok) {
    return reject(base, limits.rejection);
  }

  const props = input.props ?? input.block.props;
  if (!isRecord(props)) {
    return reject(base, {
      code: 'invalid_input',
      path: 'props',
      message: 'Restricted block props must be an object.'
    });
  }

  const state = input.state ?? {};
  if (!isRecord(state)) {
    return reject(base, {
      code: 'invalid_input',
      path: 'state',
      message: 'Restricted block state must be an object.'
    });
  }

  if (!isRecord(input.contextSnapshot)) {
    return reject(base, {
      code: 'invalid_input',
      path: 'contextSnapshot',
      message: 'Restricted block contextSnapshot must be an object.'
    });
  }

  const policy = createPolicy(input.catalogEntry, input.limits);

  return {
    ok: true,
    request: {
      requestId: createRequestId(input.block.id, codeRef),
      blockId: input.block.id,
      source: input.code,
      inputs: { ...(input.inputs ?? {}) },
      props: { ...props },
      state: { ...state },
      contextSnapshot: { ...input.contextSnapshot },
      limits: limits.value,
      allowedImports: input.catalogEntry.codeCapabilities?.allowedImports ?? []
    },
    schemaValidationOptions: {
      maxDepth: limits.value.maxRenderDepth,
      maxNodes: limits.value.maxRenderNodes,
      allowedDataPermissions: hasFrontstageBlockDataPermission(
        input.catalogEntry
      )
        ? [...(input.limits?.allowedDataOperations ?? [])]
        : [],
      allowedActions: hasFrontstageBlockActionPermission(input.catalogEntry)
        ? [...(input.limits?.allowedActions ?? [])]
        : [],
      allowedEvents: policy.allowedEvents
    },
    mediatorPolicy: policy
  };
}

function validateCatalogMatch(
  block: FrontstageBlockInstance,
  catalogEntry: NormalizedFrontstageBlockCatalogEntry
): Omit<RestrictedBlockLoaderRejection, 'ok' | 'blockId' | 'catalogId'> | null {
  if (block.catalog.providerCode !== catalogEntry.providerCode) {
    return catalogMismatch(
      'block.catalog.providerCode',
      'Restricted block provider does not match the catalog entry.'
    );
  }

  if (block.catalog.installationId !== catalogEntry.installationId) {
    return catalogMismatch(
      'block.catalog.installationId',
      'Restricted block installation does not match the catalog entry.'
    );
  }

  if (block.contribution.pluginId !== catalogEntry.pluginId) {
    return catalogMismatch(
      'block.contribution.pluginId',
      'Restricted block plugin does not match the catalog entry.'
    );
  }

  if (block.contribution.pluginVersion !== catalogEntry.pluginVersion) {
    return catalogMismatch(
      'block.contribution.pluginVersion',
      'Restricted block plugin version does not match the catalog entry.'
    );
  }

  if (block.contribution.code !== catalogEntry.contributionCode) {
    return catalogMismatch(
      'block.contribution.code',
      'Restricted block contribution does not match the catalog entry.'
    );
  }

  return null;
}

function validateRuntime(
  block: FrontstageBlockInstance,
  catalogEntry: NormalizedFrontstageBlockCatalogEntry
): Omit<RestrictedBlockLoaderRejection, 'ok' | 'blockId' | 'catalogId'> | null {
  if (!isFrontstageBlockRestrictedRuntime(catalogEntry)) {
    return {
      code: 'unsupported_runtime',
      path: 'catalogEntry.runtimeKind',
      message: `Restricted block runtime is unsupported: ${catalogEntry.runtimeKind}.`
    };
  }

  if (block.runtime.kind !== catalogEntry.runtimeKind) {
    return {
      code: 'unsupported_runtime',
      path: 'block.runtime.kind',
      message: `Restricted block runtime does not match the catalog entry: ${block.runtime.kind}.`
    };
  }

  return null;
}

function normalizeLimits(limits: RestrictedBlockLoaderLimits | undefined):
  | { ok: true; value: JsBlockRuntimeLimits }
  | {
      ok: false;
      rejection: Omit<
        RestrictedBlockLoaderRejection,
        'ok' | 'blockId' | 'catalogId'
      >;
    } {
  if (!limits) {
    return {
      ok: false,
      rejection: {
        code: 'missing_limits',
        path: 'limits',
        message: 'Restricted block runtime limits are required.'
      }
    };
  }

  if (!isPositiveNumber(limits.timeoutMs)) {
    return invalidLimit(
      'limits.timeoutMs',
      'Restricted block timeoutMs must be a positive number.'
    );
  }

  if (
    limits.maxRenderDepth !== undefined &&
    !isPositiveNumber(limits.maxRenderDepth)
  ) {
    return invalidLimit(
      'limits.maxRenderDepth',
      'Restricted block maxRenderDepth must be a positive number.'
    );
  }

  if (
    limits.maxRenderNodes !== undefined &&
    !isPositiveNumber(limits.maxRenderNodes)
  ) {
    return invalidLimit(
      'limits.maxRenderNodes',
      'Restricted block maxRenderNodes must be a positive number.'
    );
  }

  return {
    ok: true,
    value: {
      timeoutMs: limits.timeoutMs,
      maxRenderDepth: limits.maxRenderDepth,
      maxRenderNodes: limits.maxRenderNodes
    }
  };
}

function createPolicy(
  catalogEntry: NormalizedFrontstageBlockCatalogEntry,
  limits: RestrictedBlockLoaderLimits | undefined
): BlockContextMediatorPolicy {
  const allowedEvents = hasFrontstageBlockEventPermission(catalogEntry)
    ? [...(limits?.allowedEvents ?? [])]
    : [];

  return {
    allowedEvents,
    allowedInterfaces: [...(limits?.allowedInterfaces ?? [])],
    maxEventChainDepth: limits?.maxEventChainDepth
  };
}

function createRequestId(blockId: string, codeRef: string): string {
  return `restricted-block:${blockId}:${codeRef}`;
}

function normalizeRequiredString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isPositiveNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0;
}

function invalidLimit(
  path: string,
  message: string
): ReturnType<typeof normalizeLimits> {
  return {
    ok: false,
    rejection: {
      code: 'invalid_input',
      path,
      message
    }
  };
}

function catalogMismatch(
  path: string,
  message: string
): Omit<RestrictedBlockLoaderRejection, 'ok' | 'blockId' | 'catalogId'> {
  return {
    code: 'catalog_mismatch',
    path,
    message
  };
}

function getRejectionBase(
  block: FrontstageBlockInstance,
  catalogEntry: NormalizedFrontstageBlockCatalogEntry
): Pick<RestrictedBlockLoaderRejection, 'blockId' | 'catalogId'> {
  return {
    blockId: block.id,
    catalogId: catalogEntry.id
  };
}

function reject(
  base: Pick<RestrictedBlockLoaderRejection, 'blockId' | 'catalogId'>,
  rejection: Omit<
    RestrictedBlockLoaderRejection,
    'ok' | 'blockId' | 'catalogId'
  >
): RestrictedBlockLoaderRejection {
  return {
    ok: false,
    ...base,
    ...rejection
  };
}
