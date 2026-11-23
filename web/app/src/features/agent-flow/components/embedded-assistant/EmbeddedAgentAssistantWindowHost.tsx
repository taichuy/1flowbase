import { lazy, Suspense } from 'react';
import { createPortal } from 'react-dom';
import type { ConsoleAssistantClientTools } from '@1flowbase/api-client';

import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import { appI18n, applicationI18nLoader } from '../../../../shared/i18n/app-i18n';
import { resolveAppLocale } from '../../../../shared/i18n/locales';
import { WindowWorkspaceWindow } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { useWindowWorkspace } from '../../../../shared/ui/window-workspace/window-workspace-context';
import { ASSISTANT_WINDOW_ID } from './assistant-window-geometry';

const AgentFlowDockPanel = lazy(() =>
  import('../editor/AgentFlowDockPanel').then((module) => ({
    default: module.AgentFlowDockPanel
  }))
);

const EmbeddedAgentAssistantPreview = lazy(async () => {
  const [module] = await Promise.all([
    import('./EmbeddedAgentAssistantPreview'),
    applicationI18nLoader.load(resolveAppLocale(appI18n.language), [
      'agentFlow',
      'workflow',
      'applications'
    ])
  ]);
  return { default: module.EmbeddedAgentAssistantPreview };
});

export function EmbeddedAgentAssistantWindowHost({
  clientTools,
  open,
  pageKey,
  label,
  onClose
}: {
  clientTools?: ConsoleAssistantClientTools;
  open: boolean;
  pageKey: string;
  label: string;
  onClose: () => void;
}) {
  const {
    activate,
    setRect,
    state: windowWorkspaceState
  } = useWindowWorkspace();
  const windowEntry = windowWorkspaceState.windows.find(
    (entry) => entry.id === ASSISTANT_WINDOW_ID
  );

  return (
    <Suspense
      fallback={
        open && windowEntry
          ? createPortal(
              <WindowWorkspaceWindow
                active={
                  windowEntry.z_index ===
                  Math.max(
                    ...windowWorkspaceState.windows.map(
                      (entry) => entry.z_index
                    )
                  )
                }
                bodyClassName="embedded-agent-assistant-window-shell__body"
                className="embedded-agent-assistant-window-shell"
                dragHandleSelector=".agent-flow-editor__dock-panel-header"
                initialRect={() => windowEntry.rect}
                minHeight={320}
                minWidth={400}
                rect={windowEntry.rect}
                resizeEdges={[]}
                resizeLabel={() => label}
                testId="embedded-agent-assistant-window-shell"
                title={label}
                zIndex={1050 + windowEntry.z_index}
                onActivate={() => activate(ASSISTANT_WINDOW_ID)}
                onRectChange={(rect) => setRect(ASSISTANT_WINDOW_ID, rect)}
              >
                <Suspense
                  fallback={
                    <LoadingState compact className="loading-state--panel" />
                  }
                >
                  <AgentFlowDockPanel title={label} onClose={onClose}>
                    <LoadingState compact className="loading-state--panel" />
                  </AgentFlowDockPanel>
                </Suspense>
              </WindowWorkspaceWindow>,
              document.body
            )
          : null
      }
    >
      <EmbeddedAgentAssistantPreview
        clientTools={clientTools}
        open={open}
        pageKey={pageKey}
        onClose={onClose}
      />
    </Suspense>
  );
}
