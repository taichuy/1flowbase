import type {
  FrontstagePageContent,
  FrontstagePageContentNode,
  SaveFrontstageTabDocumentInput
} from '../api/page-content';

export type FrontstagePageDocumentDiagnosticSeverity = 'warning' | 'error';

export interface FrontstagePageDocumentDiagnostic {
  severity: FrontstagePageDocumentDiagnosticSeverity;
  code: string;
  path: string;
  message: string;
}

export interface FrontstageBlockCatalogRef {
  providerCode: string | null;
  installationId: string | null;
}

export interface FrontstageBlockContributionRef {
  pluginId: string | null;
  pluginVersion: string | null;
  code: string;
}

export interface FrontstageBlockRuntimeHint {
  kind: string;
  entry: string | null;
  hint: string;
  code_template_version?: string;
  code_template_language?: 'jsx' | 'tsx';
}

export type FrontstageBlockLayout = Record<string, unknown> & {
  order: number;
};

export type FrontstageBlockHeightMode = 'auto' | 'fixed';
export type FrontstagePageLayoutMode = 'auto' | 'free';

export interface FrontstageBlockPresentation {
  heightMode: FrontstageBlockHeightMode;
  height: number | null;
}

export interface FrontstageBlockInterfaceBinding {
  alias: string;
  operation_id: string;
  schema_digest: string;
  scope: string;
  risk_level: string;
}

export interface FrontstageBlockInstance {
  id: string;
  rendererVersion: string | null;
  sourceId: string | null;
  codeRef: string;
  sourceCodeRef: string | null;
  catalog: FrontstageBlockCatalogRef;
  contribution: FrontstageBlockContributionRef;
  props: Record<string, unknown>;
  interfaces?: FrontstageBlockInterfaceBinding[];
  presentation: FrontstageBlockPresentation;
  layout: FrontstageBlockLayout;
  order: number;
  runtime: FrontstageBlockRuntimeHint;
}

export interface FrontstagePageDocument {
  page: FrontstagePageContentNode;
  rootUid: string;
  layoutMode: FrontstagePageLayoutMode;
  blocks: FrontstageBlockInstance[];
  isEmpty: boolean;
  diagnostics: FrontstagePageDocumentDiagnostic[];
}

function resolvePageLayoutMode(
  content: FrontstagePageContent
): FrontstagePageLayoutMode {
  if (!isRecord(content.document.payload)) return 'auto';
  return content.document.payload['x-layout-mode'] === 'free' ? 'free' : 'auto';
}

interface FrontstageBlockPayload {
  id: string;
  renderer_version: string | null;
  codeRef: string;
  catalog: FrontstageBlockCatalogRef;
  contribution: FrontstageBlockContributionRef;
  props: Record<string, unknown>;
  interfaces: FrontstageBlockInterfaceBinding[];
  'x-presentation': FrontstageBlockPresentation;
  'x-layout': FrontstageBlockLayout;
  runtime: FrontstageBlockRuntimeHint;
}

type PayloadSource = 'document';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function asOptionalString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0
    ? value
    : null;
}

function getFirstString(
  source: Record<string, unknown>,
  keys: string[]
): string | null {
  for (const key of keys) {
    const value = asOptionalString(source[key]);
    if (value) {
      return value;
    }
  }

  return null;
}

function pushDiagnostic(
  diagnostics: FrontstagePageDocumentDiagnostic[],
  diagnostic: FrontstagePageDocumentDiagnostic
) {
  diagnostics.push(diagnostic);
}

function resolveRootUid(content: FrontstagePageContent): string {
  return asOptionalString(content.document.rootUid) ?? content.page.id;
}

function getPayloadRecord(
  payload: unknown,
  source: PayloadSource,
  diagnostics: FrontstagePageDocumentDiagnostic[]
): Record<string, unknown> | null {
  if (payload === undefined || payload === null) {
    return {};
  }

  if (isRecord(payload)) {
    return payload;
  }

  pushDiagnostic(diagnostics, {
    severity: 'error',
    code: 'invalid_payload',
    path: `${source}.payload`,
    message: `Frontstage ${source} payload must be an object.`
  });
  return null;
}

function selectBlockPayloads(
  content: FrontstagePageContent,
  diagnostics: FrontstagePageDocumentDiagnostic[]
): { blocks: unknown[]; source: PayloadSource | null } {
  const documentPayload = getPayloadRecord(
    content.document.payload,
    'document',
    diagnostics
  );

  return Array.isArray(documentPayload?.blocks)
    ? { blocks: documentPayload.blocks, source: 'document' }
    : { blocks: [], source: null };
}

function toUniqueValue(
  preferredValue: string,
  usedValues: Set<string>
): string {
  if (!usedValues.has(preferredValue)) {
    usedValues.add(preferredValue);
    return preferredValue;
  }

  let suffix = 2;
  let candidate = `${preferredValue}-${suffix}`;
  while (usedValues.has(candidate)) {
    suffix += 1;
    candidate = `${preferredValue}-${suffix}`;
  }

  usedValues.add(candidate);
  return candidate;
}

function normalizeLayout(
  block: Record<string, unknown>,
  order: number,
  path: string,
  diagnostics: FrontstagePageDocumentDiagnostic[]
): FrontstageBlockLayout {
  const layoutKey = Object.hasOwn(block, 'x-layout') ? 'x-layout' : 'layout';
  const rawLayout = block[layoutKey];
  if (rawLayout === undefined || rawLayout === null) {
    return { order };
  }

  if (!isRecord(rawLayout)) {
    pushDiagnostic(diagnostics, {
      severity: 'warning',
      code: 'invalid_block_layout',
      path: `${path}.${layoutKey}`,
      message: 'Frontstage block layout must be an object.'
    });
    return { order };
  }

  const layoutOrder =
    typeof rawLayout.order === 'number' && Number.isFinite(rawLayout.order)
      ? rawLayout.order
      : order;

  return {
    ...rawLayout,
    order: layoutOrder
  };
}

function normalizeProps(
  block: Record<string, unknown>,
  path: string,
  diagnostics: FrontstagePageDocumentDiagnostic[]
): Record<string, unknown> {
  const rawProps = block.props;
  if (rawProps === undefined || rawProps === null) {
    return {};
  }

  if (isRecord(rawProps)) {
    return rawProps;
  }

  pushDiagnostic(diagnostics, {
    severity: 'warning',
    code: 'invalid_block_props',
    path: `${path}.props`,
    message: 'Frontstage block props must be an object.'
  });
  return {};
}

function normalizeInterfaces(
  block: Record<string, unknown>,
  path: string,
  diagnostics: FrontstagePageDocumentDiagnostic[]
): FrontstageBlockInterfaceBinding[] {
  if (block.interfaces === undefined || block.interfaces === null) {
    return [];
  }
  if (!Array.isArray(block.interfaces)) {
    pushDiagnostic(diagnostics, {
      severity: 'warning',
      code: 'invalid_block_interfaces',
      path: `${path}.interfaces`,
      message: 'Frontstage block interfaces must be an array.'
    });
    return [];
  }

  const bindings: FrontstageBlockInterfaceBinding[] = [];
  const aliases = new Set<string>();
  for (const [index, value] of block.interfaces.entries()) {
    if (!isRecord(value)) continue;
    const alias = asOptionalString(value.alias);
    const operationId = asOptionalString(value.operation_id);
    const schemaDigest = asOptionalString(value.schema_digest);
    const scope = asOptionalString(value.scope);
    const riskLevel = asOptionalString(value.risk_level);
    if (
      !alias ||
      !operationId ||
      !schemaDigest ||
      !scope ||
      !riskLevel ||
      aliases.has(alias)
    ) {
      pushDiagnostic(diagnostics, {
        severity: 'warning',
        code: 'invalid_block_interface',
        path: `${path}.interfaces.${index}`,
        message: 'Frontstage block interface binding is invalid or duplicated.'
      });
      continue;
    }
    aliases.add(alias);
    bindings.push({
      alias,
      operation_id: operationId,
      schema_digest: schemaDigest,
      scope,
      risk_level: riskLevel
    });
  }
  return bindings;
}

function normalizePresentation(
  block: Record<string, unknown>,
  path: string,
  diagnostics: FrontstagePageDocumentDiagnostic[]
): FrontstageBlockPresentation {
  const rawPresentation = block['x-presentation'];
  if (rawPresentation === undefined || rawPresentation === null) {
    return { heightMode: 'auto', height: null };
  }

  if (!isRecord(rawPresentation)) {
    pushDiagnostic(diagnostics, {
      severity: 'warning',
      code: 'invalid_block_presentation',
      path: `${path}.x-presentation`,
      message: 'Frontstage block presentation must be an object.'
    });
    return { heightMode: 'auto', height: null };
  }

  if (rawPresentation.heightMode !== 'fixed') {
    return { heightMode: 'auto', height: null };
  }

  const height = rawPresentation.height;
  if (typeof height !== 'number' || !Number.isFinite(height) || height < 120) {
    pushDiagnostic(diagnostics, {
      severity: 'warning',
      code: 'invalid_fixed_block_height',
      path: `${path}.x-presentation.height`,
      message: 'Fixed block height must be at least 120 pixels.'
    });
    return { heightMode: 'auto', height: null };
  }

  return { heightMode: 'fixed', height };
}

function normalizeRuntime(
  block: Record<string, unknown>,
  blockIndex: number,
  diagnostics: FrontstagePageDocumentDiagnostic[]
): FrontstageBlockRuntimeHint {
  const rawRuntime = block.runtime;
  if (typeof rawRuntime === 'string' && rawRuntime.trim().length > 0) {
    return {
      kind: rawRuntime,
      entry: getFirstString(block, ['entry', 'runtimeEntry', 'runtime_entry']),
      hint: rawRuntime
    };
  }

  if (isRecord(rawRuntime)) {
    const kind =
      getFirstString(rawRuntime, ['kind', 'type', 'runtime']) ?? 'unknown';
    return {
      kind,
      entry:
        getFirstString(rawRuntime, ['entry', 'entrypoint', 'entry_point']) ??
        getFirstString(block, ['entry', 'runtimeEntry', 'runtime_entry']),
      hint: getFirstString(rawRuntime, ['hint']) ?? kind,
      ...(getFirstString(rawRuntime, ['code_template_version'])
        ? {
            code_template_version: getFirstString(rawRuntime, [
              'code_template_version'
            ]) as string
          }
        : {}),
      ...(getFirstString(rawRuntime, ['code_template_language']) === 'jsx' ||
      getFirstString(rawRuntime, ['code_template_language']) === 'tsx'
        ? {
            code_template_language: getFirstString(rawRuntime, [
              'code_template_language'
            ]) as 'jsx' | 'tsx'
          }
        : {})
    };
  }

  pushDiagnostic(diagnostics, {
    severity: 'warning',
    code: 'missing_runtime',
    path: `blocks.${blockIndex}.runtime`,
    message: 'Frontstage block runtime hint is missing.'
  });

  return {
    kind: 'unknown',
    entry: getFirstString(block, ['entry', 'runtimeEntry', 'runtime_entry']),
    hint: 'unknown'
  };
}

function normalizeRendererVersion(
  block: Record<string, unknown>,
  path: string,
  diagnostics: FrontstagePageDocumentDiagnostic[]
): string | null {
  const value = block.renderer_version;

  if (typeof value === 'string' && value.trim().length > 0) {
    return value;
  }

  pushDiagnostic(diagnostics, {
    severity: 'error',
    code:
      value === undefined
        ? 'missing_renderer_version'
        : 'invalid_renderer_version',
    path: `${path}.renderer_version`,
    message:
      value === undefined
        ? 'Frontstage block renderer_version is required.'
        : 'Frontstage block renderer_version must be a non-empty string.'
  });
  return null;
}

function normalizeBlock(
  block: Record<string, unknown>,
  blockIndex: number,
  usedIds: Set<string>,
  usedCodeRefs: Set<string>,
  pendingRuntimeDiagnostics: FrontstagePageDocumentDiagnostic[],
  diagnostics: FrontstagePageDocumentDiagnostic[]
): FrontstageBlockInstance {
  const path = `blocks.${blockIndex}`;
  const fallbackId = `block-${blockIndex + 1}`;
  const sourceId = getFirstString(block, ['id', 'uid', 'blockId', 'block_id']);
  if (!sourceId) {
    pushDiagnostic(diagnostics, {
      severity: 'warning',
      code: 'missing_block_id',
      path,
      message: 'Frontstage block is missing a stable id.'
    });
  }

  const preferredId = sourceId ?? fallbackId;
  const id = toUniqueValue(preferredId, usedIds);
  if (sourceId && id !== sourceId) {
    pushDiagnostic(diagnostics, {
      severity: 'warning',
      code: 'duplicate_block_id',
      path,
      message: `Frontstage block id "${sourceId}" is duplicated.`
    });
  }

  const sourceCodeRef = getFirstString(block, [
    'codeRef',
    'code_ref',
    'codeReference',
    'code_reference'
  ]);
  if (!sourceCodeRef) {
    pushDiagnostic(diagnostics, {
      severity: 'warning',
      code: 'missing_code_ref',
      path,
      message: 'Frontstage block is missing a codeRef.'
    });
  }

  const preferredCodeRef = sourceCodeRef ?? `${id}-code`;
  const codeRef = toUniqueValue(preferredCodeRef, usedCodeRefs);
  if (sourceCodeRef && codeRef !== sourceCodeRef) {
    pushDiagnostic(diagnostics, {
      severity: 'warning',
      code: 'duplicate_code_ref',
      path,
      message: `Frontstage block codeRef "${sourceCodeRef}" is duplicated.`
    });
  }

  const rawCatalog = isRecord(block.catalog) ? block.catalog : block;
  const rawContribution = isRecord(block.contribution)
    ? block.contribution
    : block;
  const contributionCode =
    getFirstString(rawContribution, [
      'code',
      'contributionCode',
      'contribution_code'
    ]) ?? 'unknown';

  if (contributionCode === 'unknown') {
    pushDiagnostic(diagnostics, {
      severity: 'warning',
      code: 'missing_contribution',
      path,
      message: 'Frontstage block is missing a contribution identifier.'
    });
  }

  const props = normalizeProps(block, path, diagnostics);
  const interfaces = normalizeInterfaces(block, path, diagnostics);
  const presentation = normalizePresentation(block, path, diagnostics);
  const layout = normalizeLayout(block, blockIndex, path, diagnostics);
  const rendererVersion = normalizeRendererVersion(block, path, diagnostics);
  const runtimeDiagnostics: FrontstagePageDocumentDiagnostic[] = [];
  const runtime = normalizeRuntime(block, blockIndex, runtimeDiagnostics);
  pendingRuntimeDiagnostics.push(...runtimeDiagnostics);

  return {
    id,
    rendererVersion,
    sourceId,
    codeRef,
    sourceCodeRef,
    catalog: {
      providerCode: getFirstString(rawCatalog, [
        'providerCode',
        'provider_code'
      ]),
      installationId: getFirstString(rawCatalog, [
        'installationId',
        'installation_id'
      ])
    },
    contribution: {
      pluginId: getFirstString(rawContribution, ['pluginId', 'plugin_id']),
      pluginVersion: getFirstString(rawContribution, [
        'pluginVersion',
        'plugin_version'
      ]),
      code: contributionCode
    },
    props,
    interfaces,
    presentation,
    layout,
    order: layout.order,
    runtime
  };
}

export function createFrontstagePageDocument(
  content: FrontstagePageContent
): FrontstagePageDocument {
  const diagnostics: FrontstagePageDocumentDiagnostic[] = [];
  const { blocks: rawBlocks, source } = selectBlockPayloads(
    content,
    diagnostics
  );
  const usedIds = new Set<string>();
  const usedCodeRefs = new Set<string>();
  const runtimeDiagnostics: FrontstagePageDocumentDiagnostic[] = [];
  const blocks: FrontstageBlockInstance[] = [];

  rawBlocks.forEach((rawBlock, index) => {
    if (!isRecord(rawBlock)) {
      pushDiagnostic(diagnostics, {
        severity: 'warning',
        code: 'invalid_block',
        path: `${source ?? 'document'}.payload.blocks.${index}`,
        message: 'Frontstage block instance must be an object.'
      });
      return;
    }

    blocks.push(
      normalizeBlock(
        rawBlock,
        index,
        usedIds,
        usedCodeRefs,
        runtimeDiagnostics,
        diagnostics
      )
    );
  });

  diagnostics.push(...runtimeDiagnostics);

  return {
    page: content.page,
    rootUid: resolveRootUid(content),
    layoutMode: resolvePageLayoutMode(content),
    blocks,
    isEmpty: blocks.length === 0,
    diagnostics
  };
}

function createPayloadRecord(payload: unknown): Record<string, unknown> {
  return isRecord(payload) ? { ...payload } : {};
}

function createBlockPayload(
  block: FrontstageBlockInstance
): FrontstageBlockPayload {
  return {
    id: block.id,
    renderer_version: block.rendererVersion,
    codeRef: block.codeRef,
    catalog: { ...block.catalog },
    contribution: { ...block.contribution },
    props: { ...block.props },
    interfaces: (block.interfaces ?? []).map((binding) => ({ ...binding })),
    'x-presentation': {
      ...(block.presentation ?? { heightMode: 'auto', height: null })
    },
    'x-layout': {
      ...block.layout,
      order: block.order
    },
    runtime: { ...block.runtime }
  };
}

function createPayloadWithBlocks(
  payload: unknown,
  blocks: FrontstageBlockPayload[],
  layoutMode: FrontstagePageLayoutMode
): Record<string, unknown> {
  return {
    ...createPayloadRecord(payload),
    'x-layout-mode': layoutMode,
    blocks
  };
}

export function createFrontstagePageDocumentSaveInput(
  content: FrontstagePageContent,
  document: FrontstagePageDocument
): SaveFrontstageTabDocumentInput {
  return {
    payload: createPayloadWithBlocks(
      content.document.payload,
      document.blocks.map(createBlockPayload),
      document.layoutMode
    )
  };
}
