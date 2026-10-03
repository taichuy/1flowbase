import {
  cloneElement,
  useLayoutEffect,
  useReducer,
  useRef,
  type ReactElement
} from 'react';
import type { FrontStagePageProps } from '../page-props';
import type { FrontStageTreeNode } from '../../../lib/page-tree';

// Match React Query's default inactive lifetime, without a navigation-count limit.
export const FRONTSTAGE_SESSION_IDLE_MS = 5 * 60 * 1000;

type Entry = {
  element: ReactElement<FrontStagePageProps>;
  inactiveSince: number | null;
};

function collectPageIds(nodes: FrontStageTreeNode[], ids = new Set<string>()) {
  for (const node of nodes) {
    if (node.kind === 'page') ids.add(node.id);
    if (node.kind === 'group') collectPageIds(node.children ?? [], ids);
  }
  return ids;
}

/** Kept inside the workspace/session key boundary: disposal unmounts native roots. */
export function RetainedFrontstagePages({
  activeKey,
  pageTree,
  children
}: {
  activeKey: string;
  pageTree?: FrontStageTreeNode[];
  children: ReactElement<FrontStagePageProps>;
}) {
  const entries = useRef(new Map<string, Entry>());
  const activeKeyRef = useRef(activeKey);
  activeKeyRef.current = activeKey;
  const [, wakeForExpiry] = useReducer((value: number) => value + 1, 0);
  const now = Date.now();
  const props = children.props;
  const allowedPages = pageTree ? collectPageIds(pageTree) : null;
  const failed = Boolean(
    props.hasPageTreeLoadError ||
    props.hasPageContentLoadError ||
    props.isPageContentPermissionDenied ||
    props.hasBlockRootsLoadError ||
    props.hasBlockRuntimeLoadError ||
    props.isBlockRuntimePermissionDenied
  );
  for (const [key, entry] of entries.current) {
    const entryPageId = entry.element.props.pageId;
    if (
      (allowedPages && entryPageId && !allowedPages.has(entryPageId)) ||
      (failed && entryPageId === props.pageId) ||
      (entry.inactiveSince !== null &&
        now - entry.inactiveSince >= FRONTSTAGE_SESSION_IDLE_MS)
    ) {
      entries.current.delete(key);
      continue;
    }
    if (key !== activeKey && entry.inactiveSince === null)
      entry.inactiveSince = now;
  }
  // Never replace a hot page snapshot with content belonging to another route.
  const ready =
    !failed &&
    Boolean(
      props.pageId &&
      (!allowedPages || allowedPages.has(props.pageId)) &&
      props.pageContent?.page.id === props.pageId &&
      props.pageContent?.tab.id === props.tabId
    );
  if (ready)
    entries.current.set(activeKey, { element: children, inactiveSince: null });
  const activeEntry = entries.current.get(activeKey);
  if (activeEntry) activeEntry.inactiveSince = null;
  useLayoutEffect(() => {
    const deadlines = [...entries.current.values()].flatMap((entry) =>
      entry.inactiveSince === null
        ? []
        : [entry.inactiveSince + FRONTSTAGE_SESSION_IDLE_MS]
    );
    if (!deadlines.length) return;
    const timer = setTimeout(
      wakeForExpiry,
      Math.max(1, Math.min(...deadlines) - Date.now())
    );
    return () => clearTimeout(timer);
  });

  return (
    <>
      {[...entries.current].map(([key, entry]) => {
        const active = key === activeKey;
        const current = active && ready ? children : entry.element;
        const guard = <Args extends unknown[]>(
          callback?: (...args: Args) => void
        ) =>
          callback
            ? (...args: Args) => {
                if (activeKeyRef.current === key && entries.current.has(key))
                  callback(...args);
              }
            : undefined;
        return (
          <div
            key={key}
            hidden={!active}
            style={{ display: active ? 'contents' : 'none' }}
          >
            {cloneElement(current, {
              runtimeActive: active && ready,
              onNavigatePage: guard(current.props.onNavigatePage),
              onNavigateTab: guard(current.props.onNavigateTab),
              onNavigateBlock: guard(current.props.onNavigateBlock)
            })}
          </div>
        );
      })}
      {!activeEntry ? cloneElement(children, { runtimeActive: !failed }) : null}
    </>
  );
}
