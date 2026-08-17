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
  description: string | null;
  schema_version: number;
  created_at: string;
  updated_at: string;
}

export interface ConsoleFrontstageBlockNode extends ConsoleFrontstageBlockNodeSummary {
  code_ref: string;
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
  source_code: string;
  source_sha256: string | null;
  dependency_lock: unknown[] | null;
}

export interface ConsoleFrontstageBlockCodeFragment {
  block_id: string;
  page_id: string;
  source_revision: string;
  source_fragment: string;
  start_line: number;
  start_column: number;
  end_line: number;
  end_column: number;
  total_lines: number;
  total_chars: number;
  next_line: number | null;
  next_column: number | null;
  truncated_by_max_chars: boolean;
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
  code_ref: string;
  source_revision: string | null;
}

export interface ConsoleFrontstageBlockRuntimeAssembly {
  layers: ConsoleFrontstageBlockRuntimeLayer[];
}

export interface ConsoleFrontstageBlockOpenTarget {
  canonical_url: string;
}

export interface CreateConsoleFrontstageBlockNodeInput {
  tab_id?: string;
  title: string;
  description?: string;
  presentation: ConsoleFrontstageBlockPresentation;
  parent_block_id: string | null;
  before_block_id: string | null;
  after_block_id: string | null;
  source_code: string;
  input_mapping?: Record<string, string>;
  output_mapping?: Record<string, string>;
  runtime_descriptor: unknown | null;
}

export interface UpdateConsoleFrontstageBlockNodeInput {
  title?: string;
  description?: string;
  presentation?: ConsoleFrontstageBlockPresentation;
  input_mapping?: Record<string, string>;
  output_mapping?: Record<string, string>;
  runtime_descriptor?: unknown;
}

export interface UpdateConsoleFrontstageBlockDescriptorsInput {
  updates: Array<{
    block_id: string;
    runtime_descriptor: unknown;
  }>;
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
  source_code: string;
  expected_source_revision?: string | null;
}

export interface ConsoleFrontstageSourceEdit {
  start_line: number;
  start_column: number;
  end_line: number;
  end_column: number;
  replacement: string;
}

export interface PatchConsoleFrontstageBlockNodeCodeInput {
  expected_source_revision: string;
  edits: ConsoleFrontstageSourceEdit[];
}

export interface ConsoleFrontstageBlockCodeFragmentQuery {
  start_line?: number;
  start_column?: number;
  line_count?: number;
  max_chars?: number;
}

export interface ConsoleFrontstageBlockListQuery {
  limit?: number;
}

export interface ConsoleFrontstageBlockRootListQuery extends ConsoleFrontstageBlockListQuery {
  tab_id: string;
}

export interface ConsoleFrontstageBlockSearchQuery {
  tab_id: string;
  query: string;
  limit?: number;
}

export interface ConsoleFrontstageBlockDescendantsQuery {
  max_depth?: number;
  limit?: number;
}
