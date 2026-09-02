import { getWindowWorkspaceViewport } from '../../../../shared/ui/window-workspace/window-workspace-geometry';
import type { WindowWorkspaceRect } from '../../../../shared/ui/window-workspace/window-workspace-state';
import { readAssistantWindowSize } from './assistant-window-size-storage';

export const ASSISTANT_WINDOW_ID = 'embedded-agent-assistant-preview';

export function initialAssistantWindowRect(): WindowWorkspaceRect {
  const viewport = getWindowWorkspaceViewport();
  const storedSize = readAssistantWindowSize();
  const width = Math.min(
    Math.max(400, storedSize?.conversationWidth ?? 560),
    Math.max(400, viewport.width - 32)
  );
  const height = storedSize
    ? Math.min(
        Math.max(320, storedSize.windowHeight),
        Math.max(320, viewport.height - 16)
      )
    : Math.min(Math.max(480, viewport.height - 24), viewport.height - 16);
  return {
    left: Math.max(8, viewport.left + viewport.width - width - 16),
    top: Math.max(viewport.top + 8, 56),
    width,
    height
  };
}
