export interface AssistantWindowSize {
  conversationWidth: number;
  windowHeight: number;
}

const ASSISTANT_WINDOW_SIZE_STORAGE_KEY =
  '1flowbase.embedded_assistant.window_size';

export function readAssistantWindowSize(): AssistantWindowSize | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const rawSize = window.localStorage.getItem(
    ASSISTANT_WINDOW_SIZE_STORAGE_KEY
  );
  if (!rawSize) {
    return null;
  }

  try {
    const size = JSON.parse(rawSize) as Partial<AssistantWindowSize>;
    return Number.isFinite(size.conversationWidth) &&
      Number(size.conversationWidth) > 0 &&
      Number.isFinite(size.windowHeight) &&
      Number(size.windowHeight) > 0
      ? {
          conversationWidth: Number(size.conversationWidth),
          windowHeight: Number(size.windowHeight)
        }
      : null;
  } catch {
    return null;
  }
}

export function writeAssistantWindowSize(size: AssistantWindowSize) {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.setItem(
    ASSISTANT_WINDOW_SIZE_STORAGE_KEY,
    JSON.stringify({
      conversationWidth: Math.round(size.conversationWidth),
      windowHeight: Math.round(size.windowHeight)
    } satisfies AssistantWindowSize)
  );
}
