export const BLOCK_CONTEXT_KEYS = [
  'currentUser',
  'workspace',
  'application',
  'page',
  'params',
  'props',
  'state',
  'patch',
  'data',
  'actions',
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

export interface BlockContextDataAccess {
  query(queryId: string, params?: BlockContextRecord): Promise<unknown>;
}

export interface BlockContextActions {
  invoke(actionId: string, params?: BlockContextRecord): Promise<unknown>;
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

export interface BlockContext {
  currentUser: BlockContextIdentity | null;
  workspace: BlockContextEntity;
  application: BlockContextEntity;
  page: BlockContextPage;
  params: BlockContextRecord;
  props: BlockContextRecord;
  state: BlockContextRecord;
  patch(patch: BlockContextRecord): void | Promise<void>;
  data: BlockContextDataAccess;
  actions: BlockContextActions;
  events: BlockContextEvents;
  theme: BlockContextTheme;
  ui: BlockContextUi;
}
