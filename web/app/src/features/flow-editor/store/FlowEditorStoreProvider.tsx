import type { ConsoleApplicationOrchestrationState } from '@1flowbase/api-client';
import { useRef, type PropsWithChildren } from 'react';

import { createFlowEditorStore } from './index';
import { FlowEditorStoreContext, type FlowEditorStore } from './provider';

export function FlowEditorStoreProvider({
  initialState,
  children
}: PropsWithChildren<{
  initialState: ConsoleApplicationOrchestrationState;
}>) {
  const storeRef = useRef<FlowEditorStore | null>(null);

  if (!storeRef.current) {
    storeRef.current = createFlowEditorStore(initialState);
  }

  return (
    <FlowEditorStoreContext.Provider value={storeRef.current}>
      {children}
    </FlowEditorStoreContext.Provider>
  );
}
