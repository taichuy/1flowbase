import { createContext, useContext, type Context } from 'react';
import { useStore } from 'zustand';

import { createFlowEditorStore, type FlowEditorState } from './index';

export type FlowEditorStore = ReturnType<typeof createFlowEditorStore>;

export const FlowEditorStoreContext: Context<FlowEditorStore | null> =
  createContext<FlowEditorStore | null>(null);

export function useFlowEditorStore<T>(selector: (state: FlowEditorState) => T) {
  const store = useContext(FlowEditorStoreContext);

  if (!store) {
    throw new Error('FlowEditorStoreProvider is missing');
  }

  return useStore(store, selector);
}
