import { useLayoutEffect } from 'react';
import { useWindowWorkspace } from '../../shared/ui/window-workspace/window-workspace-context';
import {
  ASSISTANT_WINDOW_ID,
  initialAssistantWindowRect
} from '../../features/agent-flow/components/embedded-assistant/assistant-window-geometry';
import { EmbeddedAgentAssistantPreview } from '../../features/agent-flow/components/embedded-assistant/EmbeddedAgentAssistantPreview';

export function AssistantPreviewScene() {
  const { open, close } = useWindowWorkspace();
  useLayoutEffect(() => {
    open({
      id: ASSISTANT_WINDOW_ID,
      owner: 'embedded-agent-assistant',
      parent_id: null,
      rect: initialAssistantWindowRect(),
      dirty: false
    });
    return () => close(ASSISTANT_WINDOW_ID);
  }, [open, close]);

  return (
    <EmbeddedAgentAssistantPreview
      open
      pageKey="/style-boundary/embedded-agent-assistant-preview"
      onClose={() => close(ASSISTANT_WINDOW_ID)}
    />
  );
}
