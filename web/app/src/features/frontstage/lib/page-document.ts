import type {
  FrontstagePageContent,
  FrontstagePageContentNode,
  SaveFrontstagePageContentInput
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

export interface FrontstageBlockInstance {
  id: string;
  sourceId: string | null;
  codeRef: string;
  sourceCodeRef: string | null;
  catalog: FrontstageBlockCatalogRef;
  contribution: FrontstageBlockContributionRef;
  props: Record<string, unknown>;
  layout: FrontstageBlockLayout;
  order: number;
  runtime: FrontstageBlockRuntimeHint;
}

export interface FrontstagePageDocument {
  page: FrontstagePageContentNode;
  rootUid: string;
  blocks: FrontstageBlockInstance[];
  isEmpty: boolean;
  diagnostics: FrontstagePageDocumentDiagnostic[];
}

interface FrontstageBlockPayload {
  id: string;
  codeRef: string;
  catalog: FrontstageBlockCatalogRef;
  contribution: FrontstageBlockContributionRef;
  props: Record<string, unknown>;
  'x-layout': FrontstageBlockLayout;
  runtime: FrontstageBlockRuntimeHint;
}

type PayloadSource = 'root' | 'schema';

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
  return (
    asOptionalString(content.root.uid) ??
    asOptionalString(content.schema.rootUid) ??
    content.page.id
  );
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
  const rootPayload = getPayloadRecord(
    content.root.payload,
    'root',
    diagnostics
  );
  const schemaPayload = getPayloadRecord(
    content.schema.payload,
    'schema',
    diagnostics
  );

  if (Array.isArray(rootPayload?.blocks)) {
    return { blocks: rootPayload.blocks, source: 'root' };
  }

  if (Array.isArray(schemaPayload?.blocks)) {
    return { blocks: schemaPayload.blocks, source: 'schema' };
  }

  return { blocks: [], source: null };
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
  const layout = normalizeLayout(block, blockIndex, path, diagnostics);
  const runtimeDiagnostics: FrontstagePageDocumentDiagnostic[] = [];
  const runtime = normalizeRuntime(block, blockIndex, runtimeDiagnostics);
  pendingRuntimeDiagnostics.push(...runtimeDiagnostics);

  return {
    id,
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
        path: `${source ?? 'root'}.payload.blocks.${index}`,
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
    codeRef: block.codeRef,
    catalog: { ...block.catalog },
    contribution: { ...block.contribution },
    props: { ...block.props },
    'x-layout': {
      ...block.layout,
      order: block.order
    },
    runtime: { ...block.runtime }
  };
}

function createPayloadWithBlocks(
  payload: unknown,
  blocks: FrontstageBlockPayload[]
): Record<string, unknown> {
  return {
    ...createPayloadRecord(payload),
    blocks
  };
}

export function createFrontstagePageDocumentSaveInput(
  content: FrontstagePageContent,
  document: FrontstagePageDocument
): SaveFrontstagePageContentInput {
  return {
    schema: {
      payload: createPayloadWithBlocks(
        content.schema.payload,
        document.blocks.map(createBlockPayload)
      )
    },
    root: {
      payload: createPayloadWithBlocks(
        content.root.payload,
        document.blocks.map(createBlockPayload)
      )
    }
  };
}
