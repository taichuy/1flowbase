export const BLOCK_CONTEXT_KEYS = [
  'currentUser',
  'workspace',
  'application',
  'page',
  'inputs',
  'outputs',
  'params',
  'props',
  'state',
  'patch',
  'api',
  'events',
  'theme',
  'ui'
] as const;

export type BlockContextKey = (typeof BLOCK_CONTEXT_KEYS)[number];

export type BlockContextRecord = Record<string, unknown>;

export interface BlockContextIdentity {
  id: string;
  displayName?: string;
}

export interface BlockContextEntity {
  id: string;
  name?: string;
}

export interface BlockContextPage {
  id: string;
  route: string;
  title?: string;
}

export interface BlockApiRequest {
  path?: BlockContextRecord;
  query?: BlockContextRecord;
  headers?: BlockContextRecord;
  body?: unknown;
}

export interface BlockBinaryInput {
  base64: string;
  file_name?: string;
  content_type?: string;
}

export interface BlockBinaryResource {
  bytes: Uint8Array;
  file_name: string | null;
  content_type: string;
}

export type BlockApiMethod =
  | 'GET'
  | 'POST'
  | 'PUT'
  | 'PATCH'
  | 'DELETE'
  | 'HEAD'
  | 'OPTIONS';

export interface BlockContextApi {
  get<TResponse = never>(
    path: string,
    request?: BlockApiRequest
  ): Promise<TResponse>;
  post<TResponse = never>(
    path: string,
    request?: BlockApiRequest
  ): Promise<TResponse>;
  put<TResponse = never>(
    path: string,
    request?: BlockApiRequest
  ): Promise<TResponse>;
  patch<TResponse = never>(
    path: string,
    request?: BlockApiRequest
  ): Promise<TResponse>;
  delete<TResponse = never>(
    path: string,
    request?: BlockApiRequest
  ): Promise<TResponse>;
  head<TResponse = never>(
    path: string,
    request?: BlockApiRequest
  ): Promise<TResponse>;
  options<TResponse = never>(
    path: string,
    request?: BlockApiRequest
  ): Promise<TResponse>;
  stream<TEvent = never>(
    method: BlockApiMethod,
    path: string,
    request?: BlockApiRequest
  ): AsyncIterable<TEvent>;
}

export interface BlockContextEvents {
  emit(event: string, payload?: BlockContextRecord): void;
}

export interface BlockContextOutputPublishResult {
  ok: boolean;
  stale: boolean;
  error?: string;
}

export interface BlockContextOutputs<
  TOutputs extends BlockContextRecord = BlockContextRecord
> {
  publish(
    values: TOutputs
  ): BlockContextOutputPublishResult | Promise<BlockContextOutputPublishResult>;
}

export interface BlockContextTheme {
  mode: 'light' | 'dark';
  tokens: BlockContextRecord;
}

export interface BlockContextUi {
  locale?: string;
  density?: 'compact' | 'comfortable';
}

export interface BlockContext<
  TInputs extends BlockContextRecord = BlockContextRecord,
  TOutputs extends BlockContextRecord = BlockContextRecord
> {
  currentUser: BlockContextIdentity | null;
  workspace: BlockContextEntity;
  application: BlockContextEntity | null;
  page: BlockContextPage;
  inputs: Readonly<TInputs>;
  outputs: BlockContextOutputs<TOutputs>;
  params: BlockContextRecord;
  props: BlockContextRecord;
  state: BlockContextRecord;
  patch(patch: BlockContextRecord): void | Promise<void>;
  api: BlockContextApi;
  events: BlockContextEvents;
  theme: BlockContextTheme;
  ui: BlockContextUi;
}
