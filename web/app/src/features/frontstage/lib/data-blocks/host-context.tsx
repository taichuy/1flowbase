import { createContext, useContext } from 'react';
import type { DataBlockAction, DataBlockBinding, DataBlockConfig, DataBlockFormMode, DataBlockRelation } from './types';

export interface DataBlockTarget {
  block_id: string;
  title: string;
  kind: 'table' | 'form' | 'code';
}

/** Trusted host callbacks. UI drafts and data record writes have separate owners. */
export interface DataBlockHost {
  block_id: string;
  design_mode: boolean;
  saving: boolean;
  targets: readonly DataBlockTarget[];
  saveConfig(config: DataBlockConfig): Promise<void>;
  prepareAction(action: DataBlockAction, binding: DataBlockBinding, mode: DataBlockFormMode, relation?: DataBlockRelation): Promise<DataBlockAction>;
  openAction(action: DataBlockAction, inputs?: Readonly<Record<string, string>>): void | Promise<void>;
  close(): void;
}

export const DataBlockHostContext = createContext<DataBlockHost | null>(null);

export function useDataBlockHost(): DataBlockHost | null {
  return useContext(DataBlockHostContext);
}
