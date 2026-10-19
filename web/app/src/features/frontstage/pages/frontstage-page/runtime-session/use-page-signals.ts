import { useEffect, useRef } from 'react';
import type { FrontstageBlockInstance } from '../../../lib/page-document';
import {
  createFrontstagePageSignalSession,
  FrontstageSignalRuntimeCoordinator,
  type FrontstagePageSignalSession
} from '../../../lib/page-canvas/signal-runtime';

interface SignalScope {
  pageId: string | undefined;
  session: FrontstagePageSignalSession;
}
interface CoordinatorScope {
  page: SignalScope;
  tabId: string;
  rootUid: string | null | undefined;
  coordinator: FrontstageSignalRuntimeCoordinator;
}

export function usePageSignals({ pageId, tabId, rootUid, blocks }: {
  pageId: string | undefined;
  tabId: string | undefined;
  rootUid: string | null | undefined;
  blocks: readonly FrontstageBlockInstance[] | undefined;
}) {
  const pageScope = useRef<SignalScope | null>(null);
  const coordinatorScope = useRef<CoordinatorScope | null>(null);
  if (!pageScope.current || pageScope.current.pageId !== pageId) {
    pageScope.current = { pageId, session: createFrontstagePageSignalSession() };
  }
  if (blocks === undefined || tabId === undefined) {
    coordinatorScope.current = null;
  } else if (
    !coordinatorScope.current ||
    coordinatorScope.current.page !== pageScope.current ||
    coordinatorScope.current.tabId !== tabId ||
    coordinatorScope.current.rootUid !== rootUid
  ) {
    coordinatorScope.current = {
      page: pageScope.current,
      tabId,
      rootUid,
      coordinator: new FrontstageSignalRuntimeCoordinator(blocks, tabId, pageScope.current.session)
    };
  }
  const coordinator = coordinatorScope.current?.coordinator;
  useEffect(() => {
    coordinator?.updateBlocks(blocks ?? []);
  }, [blocks, coordinator]);
  useEffect(() => () => coordinator?.dispose(), [coordinator]);
  return coordinator;
}
