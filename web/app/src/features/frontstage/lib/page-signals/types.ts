export type FrontstageSignalScope = 'tab' | 'page';

export interface FrontstageBlockOutputPort {
  name: string;
  schema: Record<string, unknown>;
}

export interface FrontstageBlockInputSource {
  block_id: string;
  output: string;
  scope: FrontstageSignalScope;
  tab_id?: string;
}

export interface FrontstageBlockInputPort {
  name: string;
  schema: Record<string, unknown>;
  source?: FrontstageBlockInputSource;
}

export interface FrontstageBlockPorts {
  inputs: FrontstageBlockInputPort[];
  outputs: FrontstageBlockOutputPort[];
}

export interface FrontstageSignalAddress {
  scope: FrontstageSignalScope;
  tab_id: string;
  block_id: string;
  output: string;
}
