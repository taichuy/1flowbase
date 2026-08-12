export type ConsoleFrontstageBlockPresentation =
  | 'page'
  | 'drawer'
  | 'modal'
  | 'inline';

export interface ConsoleFrontstageBlockNodeSummary {
  block_id: string;
  workspace_id: string;
  page_id: string;
  tab_id: string;
  parent_block_id: string | null;
  rank: string;
  presentation: ConsoleFrontstageBlockPresentation;
  title: string | null;
  schema_version: number;
  created_at: string;
  updated_at: string;
}

export interface ConsoleFrontstageBlockNode extends ConsoleFrontstageBlockNodeSummary {
  input_mapping: Record<string, string>;
  output_mapping: Record<string, string>;
  runtime_descriptor: unknown;
}

export interface ConsoleFrontstageBlockDescendant {
  node: ConsoleFrontstageBlockNodeSummary;
  depth: number;
  has_children: boolean;
  path: string[];
}

export interface ConsoleFrontstageBlockSearchResult {
  node: ConsoleFrontstageBlockNodeSummary;
  ancestors: ConsoleFrontstageBlockNodeSummary[];
}

export interface ConsoleFrontstageBlockDeleteImpact {
  affected_count: number;
}

export interface ConsoleFrontstageBlockSubtreeDeleteResult {
  deleted_count: number;
}

export interface ConsoleFrontstageBlockNodeCode {
  block_id: string;
  page_id: string;
  code: string;
  source_sha256: string;
}

export interface ConsoleFrontstageBlockRuntimeLayer {
  block_id: string;
  tab_id: string;
  parent_block_id: string | null;
  title: string | null;
  presentation: ConsoleFrontstageBlockPresentation;
  schema_version: number;
  input_mapping: Record<string, string>;
  output_mapping: Record<string, string>;
  runtime_descriptor: unknown;
  code: string;
  source_sha256: string;
}

export interface ConsoleFrontstageBlockRuntimeAssembly {
  layers: ConsoleFrontstageBlockRuntimeLayer[];
}

export interface ConsoleFrontstageBlockOpenTarget {
  canonical_url: string;
}

export interface CreateConsoleFrontstageBlockNodeInput {
  tab_id: string;
  title: string;
  presentation: ConsoleFrontstageBlockPresentation;
  parent_block_id: string | null;
  before_block_id: string | null;
  after_block_id: string | null;
  code: string;
  input_mapping?: Record<string, string>;
  output_mapping?: Record<string, string>;
  runtime_descriptor: unknown | null;
}

export interface UpdateConsoleFrontstageBlockNodeInput {
  title?: string;
  presentation?: ConsoleFrontstageBlockPresentation;
  input_mapping?: Record<string, string>;
  output_mapping?: Record<string, string>;
  runtime_descriptor?: unknown;
}

export interface MoveConsoleFrontstageBlockNodeInput {
  parent_block_id: string | null;
  before_block_id: string | null;
  after_block_id: string | null;
}

export interface DeleteConsoleFrontstageBlockSubtreeInput {
  expected_affected_count: number;
}

export interface SaveConsoleFrontstageBlockNodeCodeInput {
  code: string;
}

export interface ConsoleFrontstageBlockListQuery {
  limit?: number;
}

export interface ConsoleFrontstageBlockSearchQuery {
  query: string;
  limit?: number;
}

export interface ConsoleFrontstageBlockDescendantsQuery {
  max_depth?: number;
  limit?: number;
}
