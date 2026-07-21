export const BLOCK_CONTEXT_KEYS = [
  'currentUser',
  'workspace',
  'application',
  'page',
  'inputs',
  'params',
  'props',
  'state',
  'patch',
  'interfaces',
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

export interface BlockInterfaceRequest {
  path?: BlockContextRecord;
  query?: BlockContextRecord;
  headers?: BlockContextRecord;
  body?: unknown;
}

export interface BlockInterfaceDescriptor {
  interfaceId: string;
  schemaDigest: string;
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

export interface BlockContextInterfaces {
  call<TResponse = unknown>(
    descriptor: BlockInterfaceDescriptor,
    request?: BlockInterfaceRequest
  ): Promise<TResponse>;
  stream<TEvent = unknown>(
    descriptor: BlockInterfaceDescriptor,
    request?: BlockInterfaceRequest
  ): AsyncIterable<TEvent>;
}

export interface BlockContextEvents {
  emit(event: string, payload?: BlockContextRecord): void;
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
  TInputs extends BlockContextRecord = BlockContextRecord
> {
  currentUser: BlockContextIdentity | null;
  workspace: BlockContextEntity;
  application: BlockContextEntity;
  page: BlockContextPage;
  inputs: Readonly<TInputs>;
  params: BlockContextRecord;
  props: BlockContextRecord;
  state: BlockContextRecord;
  patch(patch: BlockContextRecord): void | Promise<void>;
  interfaces: BlockContextInterfaces;
  events: BlockContextEvents;
  theme: BlockContextTheme;
  ui: BlockContextUi;
}
