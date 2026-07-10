import type { ConsoleApplicationOrchestrationState } from '@1flowbase/api-client';
import type { ReactNode } from 'react';

import { FlowEditorStoreProvider } from '../store/FlowEditorStoreProvider';

export interface FlowEditorKernelSlots {
  toolbar?: ReactNode;
  canvas: ReactNode;
  detail?: ReactNode;
  panels?: ReactNode;
}

export function FlowEditorKernel({
  initialState,
  slots
}: {
  initialState: ConsoleApplicationOrchestrationState;
  slots: FlowEditorKernelSlots;
}) {
  return (
    <FlowEditorStoreProvider initialState={initialState}>
      {slots.toolbar}
      {slots.canvas}
      {slots.detail}
      {slots.panels}
    </FlowEditorStoreProvider>
  );
}
