import type {
  FrontstagePageContent,
  FrontstagePageContentNode
} from '../api/page-content';
import type { FrontstageBlockPorts } from './page-signals/types';

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

export interface FrontstageBlockInstance {
  id: string;
  title?: string | null;
  rendererVersion: string | null;
  sourceId: string | null;
  codeRef: string;
  sourceCodeRef: string | null;
  catalog: FrontstageBlockCatalogRef;
  contribution: FrontstageBlockContributionRef;
  props: Record<string, unknown>;
  ports?: FrontstageBlockPorts;
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

export type NormalizedFrontstagePageDocument = FrontstagePageDocument;

interface FrontstageBlockPayload {
  id: string;
  renderer_version: string | null;
  codeRef: string;
  catalog: FrontstageBlockCatalogRef;
  contribution: FrontstageBlockContributionRef;
  props: Record<string, unknown>;
  ports: FrontstageBlockPorts;
  'x-presentation': FrontstageBlockPresentation;
  'x-layout': FrontstageBlockLayout;
  runtime: FrontstageBlockRuntimeHint;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function resolveRootUid(content: FrontstagePageContent): string {
  const rootUid = content.document.rootUid;
  return typeof rootUid === 'string' && rootUid.trim().length > 0
    ? rootUid
    : content.page.id;
}

function resolvePageLayoutMode(
  content: FrontstagePageContent
): FrontstagePageLayoutMode {
  if (!isRecord(content.document.payload)) return 'auto';
  return content.document.payload['x-layout-mode'] === 'free' ? 'free' : 'auto';
}

export function createFrontstagePageDocument(
  content: FrontstagePageContent
): NormalizedFrontstagePageDocument {
  return {
    page: content.page,
    rootUid: resolveRootUid(content),
    layoutMode: resolvePageLayoutMode(content),
    blocks: [],
    isEmpty: true,
    diagnostics: []
  };
}

export function createFrontstageBlockRuntimeDescriptor(
  block: FrontstageBlockInstance
): FrontstageBlockPayload {
  return {
    id: block.id,
    renderer_version: block.rendererVersion,
    codeRef: block.codeRef,
    catalog: { ...block.catalog },
    contribution: { ...block.contribution },
    props: { ...block.props },
    ports: {
      inputs: (block.ports?.inputs ?? []).map((port) => ({
        ...port,
        schema: { ...port.schema },
        ...(port.source ? { source: { ...port.source } } : {})
      })),
      outputs: (block.ports?.outputs ?? []).map((port) => ({
        ...port,
        schema: { ...port.schema }
      }))
    },
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
