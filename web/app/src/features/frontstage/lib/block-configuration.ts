import type { NormalizedFrontstageBlockCatalogEntry } from './block-catalog';
import type { FrontstageBlockInstance } from './page-document';
import type { RestrictedBlockLoaderLimits } from './restricted-block-loader';

export const FRONTSTAGE_BLOCK_CONFIGURATION_SECTION_IDS = [
  'basic',
  'data',
  'code',
  'context',
  'limits'
] as const;

export type FrontstageBlockConfigurationSectionId =
  (typeof FRONTSTAGE_BLOCK_CONFIGURATION_SECTION_IDS)[number];

export type FrontstageBlockConfigurationOperation =
  | 'query'
  | 'create'
  | 'update'
  | 'delete';

export interface FrontstageBlockConfigurationInput {
  block: FrontstageBlockInstance;
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  limits: RestrictedBlockLoaderLimits;
}

export interface FrontstageBlockConfigurationTextValue {
  value: string | null;
  placeholder: string;
}

export interface FrontstageBlockBasicConfiguration {
  blockId: string;
  sourceId: string | null;
  title: FrontstageBlockConfigurationTextValue;
  description: FrontstageBlockConfigurationTextValue;
  width: unknown;
  height: unknown;
  order: number | null;
  codeRef: string;
  sourceCodeRef: string | null;
  rawProps: Record<string, unknown>;
  rawLayout: Record<string, unknown>;
}

export interface FrontstageBlockDataOperationConfiguration {
  enabled: boolean;
}

export interface FrontstageBlockDataConfiguration {
  model: string | null;
  fields: unknown[];
  operations: Record<
    FrontstageBlockConfigurationOperation,
    FrontstageBlockDataOperationConfiguration
  >;
  filter: unknown;
  sort: unknown;
  pagination: unknown;
  rawConfig: unknown;
}

export interface FrontstageBlockCodeConfiguration {
  codeRef: string;
  sourceCodeRef: string | null;
  runtime: FrontstageBlockInstance['runtime'];
  contribution: {
    pluginId: string | null;
    pluginVersion: string | null;
    code: string;
    catalogId: string | null;
    catalogTitle: string | null;
    providerCode: string | null;
    installationId: string | null;
  };
  template: {
    id: string | null;
    raw: unknown;
  };
}

export interface FrontstageBlockContextBoundary {
  path: string;
  available: boolean;
}

export interface FrontstageBlockContextDataBoundary
  extends FrontstageBlockContextBoundary {
  models: string[];
  operations: RestrictedBlockLoaderLimits['allowedDataOperations'] extends
    | readonly (infer Operation)[]
    | undefined
    ? Operation[]
    : string[];
}

export interface FrontstageBlockContextListBoundary
  extends FrontstageBlockContextBoundary {
  allowed: string[];
}

export interface FrontstageBlockContextConfiguration {
  catalog: {
    available: boolean;
    primitives: NormalizedFrontstageBlockCatalogEntry['contextContract']['primitives'];
    inputSchema: Record<string, unknown>;
  };
  ctx: {
    currentUser: FrontstageBlockContextBoundary;
    page: FrontstageBlockContextBoundary;
    params: FrontstageBlockContextBoundary;
    props: FrontstageBlockContextBoundary;
    state: FrontstageBlockContextBoundary;
    data: FrontstageBlockContextDataBoundary;
    actions: FrontstageBlockContextListBoundary;
    events: FrontstageBlockContextListBoundary;
  };
}

export interface FrontstageBlockLimitsConfiguration {
  timeoutMs: number | null;
  maxRenderDepth: number | null;
  maxRenderNodes: number | null;
  maxEventChainDepth: number | null;
  allowedActions: string[];
  allowedEvents: string[];
  allowedQueries: string[];
  allowedDataOperations: FrontstageBlockContextDataBoundary['operations'];
}

export type FrontstageBlockConfigurationSection =
  | {
      id: 'basic';
      title: 'Basic';
      model: FrontstageBlockBasicConfiguration;
    }
  | {
      id: 'data';
      title: 'Data';
      model: FrontstageBlockDataConfiguration;
    }
  | {
      id: 'code';
      title: 'Code';
      model: FrontstageBlockCodeConfiguration;
    }
  | {
      id: 'context';
      title: 'Context';
      model: FrontstageBlockContextConfiguration;
    }
  | {
      id: 'limits';
      title: 'Limits';
      model: FrontstageBlockLimitsConfiguration;
    };

export interface FrontstageBlockConfigurationModel {
  blockId: string;
  codeRef: string;
  catalogId: string | null;
  sections: FrontstageBlockConfigurationSection[];
}

const dataOperations = ['query', 'create', 'update', 'delete'] as const;

export function createFrontstageBlockConfigurationModel({
  block,
  catalogEntry,
  limits
}: FrontstageBlockConfigurationInput): FrontstageBlockConfigurationModel {
  return {
    blockId: block.id,
    codeRef: block.codeRef,
    catalogId: catalogEntry?.id ?? null,
    sections: [
      {
        id: 'basic',
        title: 'Basic',
        model: createBasicConfiguration(block, catalogEntry)
      },
      {
        id: 'data',
        title: 'Data',
        model: createDataConfiguration(block)
      },
      {
        id: 'code',
        title: 'Code',
        model: createCodeConfiguration(block, catalogEntry)
      },
      {
        id: 'context',
        title: 'Context',
        model: createContextConfiguration(catalogEntry, limits)
      },
      {
        id: 'limits',
        title: 'Limits',
        model: createLimitsConfiguration(limits)
      }
    ]
  };
}

function createBasicConfiguration(
  block: FrontstageBlockInstance,
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null
): FrontstageBlockBasicConfiguration {
  return {
    blockId: block.id,
    sourceId: block.sourceId,
    title: {
      value: readString(block.props.title),
      placeholder: catalogEntry?.title ?? 'Untitled block'
    },
    description: {
      value: readString(block.props.description),
      placeholder: 'Describe what this block renders.'
    },
    width: cloneValue(block.layout.width ?? block.props.width ?? null),
    height: cloneValue(block.layout.height ?? block.props.height ?? null),
    order: readFiniteNumber(block.order) ?? readFiniteNumber(block.layout.order),
    codeRef: block.codeRef,
    sourceCodeRef: block.sourceCodeRef,
    rawProps: cloneRecord(block.props),
    rawLayout: cloneRecord(block.layout)
  };
}

function createDataConfiguration(
  block: FrontstageBlockInstance
): FrontstageBlockDataConfiguration {
  const rawConfig = readDataConfig(block.props);
  if (!isRecord(rawConfig)) {
    return {
      model: null,
      fields: [],
      operations: createOperationConfiguration(null),
      filter: null,
      sort: null,
      pagination: createEmptyPagination(),
      rawConfig: cloneValue(rawConfig ?? null)
    };
  }

  return {
    model: readString(rawConfig.model) ?? readString(rawConfig.modelCode),
    fields: Array.isArray(rawConfig.fields) ? cloneValue(rawConfig.fields) : [],
    operations: createOperationConfiguration(rawConfig.operations),
    filter:
      Object.hasOwn(rawConfig, 'filter') ? cloneValue(rawConfig.filter) : null,
    sort: Object.hasOwn(rawConfig, 'sort') ? cloneValue(rawConfig.sort) : null,
    pagination: Object.hasOwn(rawConfig, 'pagination')
      ? cloneValue(rawConfig.pagination)
      : createEmptyPagination(),
    rawConfig: cloneValue(rawConfig)
  };
}

function createCodeConfiguration(
  block: FrontstageBlockInstance,
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null
): FrontstageBlockCodeConfiguration {
  return {
    codeRef: block.codeRef,
    sourceCodeRef: block.sourceCodeRef,
    runtime: cloneValue(block.runtime),
    contribution: {
      pluginId: block.contribution.pluginId,
      pluginVersion: block.contribution.pluginVersion,
      code: block.contribution.code,
      catalogId: catalogEntry?.id ?? null,
      catalogTitle: catalogEntry?.title ?? null,
      providerCode: block.catalog.providerCode,
      installationId: block.catalog.installationId
    },
    template: readTemplateConfiguration(block.props)
  };
}

function createContextConfiguration(
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null,
  limits: RestrictedBlockLoaderLimits
): FrontstageBlockContextConfiguration {
  return {
    catalog: {
      available: catalogEntry !== null,
      primitives: catalogEntry
        ? cloneValue(catalogEntry.contextContract.primitives)
        : [],
      inputSchema: catalogEntry
        ? cloneRecord(catalogEntry.contextContract.inputSchema)
        : {}
    },
    ctx: {
      currentUser: createBoundary('ctx.currentUser'),
      page: createBoundary('ctx.page'),
      params: createBoundary('ctx.params'),
      props: createBoundary('ctx.props'),
      state: createBoundary('ctx.state'),
      data: {
        ...createBoundary('ctx.data'),
        models: [...(limits.allowedQueries ?? [])],
        operations: [...(limits.allowedDataOperations ?? [])]
      },
      actions: {
        ...createBoundary('ctx.actions'),
        allowed: [...(limits.allowedActions ?? [])]
      },
      events: {
        ...createBoundary('ctx.events'),
        allowed: [...(limits.allowedEvents ?? [])]
      }
    }
  };
}

function createLimitsConfiguration(
  limits: RestrictedBlockLoaderLimits
): FrontstageBlockLimitsConfiguration {
  return {
    timeoutMs: readFiniteNumber(limits.timeoutMs),
    maxRenderDepth: readFiniteNumber(limits.maxRenderDepth),
    maxRenderNodes: readFiniteNumber(limits.maxRenderNodes),
    maxEventChainDepth: readFiniteNumber(limits.maxEventChainDepth),
    allowedActions: [...(limits.allowedActions ?? [])],
    allowedEvents: [...(limits.allowedEvents ?? [])],
    allowedQueries: [...(limits.allowedQueries ?? [])],
    allowedDataOperations: [...(limits.allowedDataOperations ?? [])]
  };
}

function createBoundary(path: string): FrontstageBlockContextBoundary {
  return { path, available: true };
}

function createOperationConfiguration(
  value: unknown
): FrontstageBlockDataConfiguration['operations'] {
  const operations = {
    query: { enabled: false },
    create: { enabled: false },
    update: { enabled: false },
    delete: { enabled: false }
  };

  if (Array.isArray(value)) {
    for (const operation of dataOperations) {
      operations[operation].enabled = value.includes(operation);
    }
    return operations;
  }

  if (isRecord(value)) {
    for (const operation of dataOperations) {
      operations[operation].enabled = value[operation] === true;
    }
  }

  return operations;
}

function createEmptyPagination(): { current: null; pageSize: null } {
  return {
    current: null,
    pageSize: null
  };
}

function readDataConfig(props: Record<string, unknown>): unknown {
  if (Object.hasOwn(props, 'data')) {
    return props.data;
  }

  if (Object.hasOwn(props, 'dataConfig')) {
    return props.dataConfig;
  }

  if (isRecord(props.config) && Object.hasOwn(props.config, 'data')) {
    return props.config.data;
  }

  return null;
}

function readTemplateConfiguration(
  props: Record<string, unknown>
): FrontstageBlockCodeConfiguration['template'] {
  if (Object.hasOwn(props, 'templateId')) {
    return {
      id: readString(props.templateId),
      raw: cloneValue(props.templateId)
    };
  }

  if (Object.hasOwn(props, 'template')) {
    const template = props.template;
    return {
      id: isRecord(template) ? readString(template.id) : readString(template),
      raw: cloneValue(template)
    };
  }

  return {
    id: null,
    raw: null
  };
}

function readString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0
    ? value
    : null;
}

function readFiniteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function cloneRecord<T extends Record<string, unknown>>(value: T): T {
  return cloneValue(value);
}

function cloneValue<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((item) => cloneValue(item)) as T;
  }

  if (isPlainRecord(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, cloneValue(entry)])
    ) as T;
  }

  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  if (!isRecord(value)) {
    return false;
  }

  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
