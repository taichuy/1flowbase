import type { ConsoleAssistantClientTools } from '@1flowbase/api-client';
import { createContext, useContext, useEffect } from 'react';

import type { AssistantRefreshTarget } from './factory';

interface AssistantClientToolContextValue {
  clientTools: ConsoleAssistantClientTools;
  registerRefreshTarget(
    targetId: string,
    refresh: AssistantRefreshTarget
  ): () => void;
}

export const AssistantClientToolContext =
  createContext<AssistantClientToolContextValue | null>(null);

export function useAssistantClientTools() {
  const value = useContext(AssistantClientToolContext);
  if (!value) {
    throw new Error('Assistant client tools require AppShell ownership');
  }
  return value.clientTools;
}

export function useAssistantRefreshTarget(
  targetId: string,
  refresh: AssistantRefreshTarget
) {
  const value = useContext(AssistantClientToolContext);
  useEffect(() => {
    if (!value) {
      return;
    }
    return value.registerRefreshTarget(targetId, refresh);
  }, [refresh, targetId, value]);
}
