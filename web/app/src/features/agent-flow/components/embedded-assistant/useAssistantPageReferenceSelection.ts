import type { ConsoleAssistantPageReference } from '@1flowbase/api-client';
import { useCallback, useEffect, useRef, useState } from 'react';

const ASSISTANT_CHROME_SELECTOR =
  '.embedded-agent-assistant-preview, [data-assistant-page-reference-chrome="true"]';

function byteLength(value: string) {
  return new TextEncoder().encode(value).byteLength;
}

export function useAssistantPageReferenceSelection({
  active,
  maxBytes,
  pageKey,
  selectionHint,
  tooLargeMessage,
  unsupportedIsolatedFrameMessage
}: {
  active: boolean;
  maxBytes: number;
  pageKey: string;
  selectionHint: string;
  tooLargeMessage: (actualBytes: number, maxBytes: number) => string;
  unsupportedIsolatedFrameMessage: string;
}) {
  const [error, setError] = useState<string | null>(null);
  const [reference, setReference] =
    useState<ConsoleAssistantPageReference | null>(null);
  const [selecting, setSelecting] = useState(false);
  const previousPageKeyRef = useRef(pageKey);

  const cancelSelection = useCallback(() => setSelecting(false), []);
  const clearReference = useCallback(() => {
    setError(null);
    setReference(null);
  }, []);
  const startSelection = useCallback(() => {
    if (!active || maxBytes <= 0) return;
    setError(null);
    setSelecting(true);
  }, [active, maxBytes]);

  useEffect(() => {
    if (previousPageKeyRef.current === pageKey) return;
    previousPageKeyRef.current = pageKey;
    setSelecting(false);
    clearReference();
  }, [clearReference, pageKey]);

  useEffect(() => {
    if (active) return;
    setSelecting(false);
    clearReference();
  }, [active, clearReference]);

  useEffect(() => {
    if (!selecting) return;

    const outline = document.createElement('div');
    outline.className = 'embedded-agent-assistant-page-reference-outline';
    outline.dataset.assistantPageReferenceChrome = 'true';
    outline.dataset.testid = 'assistant-page-reference-outline';
    const hint = document.createElement('div');
    hint.className = 'embedded-agent-assistant-page-reference-hint';
    hint.dataset.assistantPageReferenceChrome = 'true';
    hint.textContent = selectionHint;
    document.body.append(outline, hint);

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = 'crosshair';
    document.body.style.userSelect = 'none';

    const isIsolatedIframe = (element: Element) => {
      if (!(element instanceof HTMLIFrameElement)) return false;
      const sandboxTokens = element
        .getAttribute('sandbox')
        ?.split(/\s+/)
        .filter(Boolean);
      return Boolean(
        sandboxTokens && !sandboxTokens.includes('allow-same-origin')
      );
    };
    const isolatedFrameOverlays = new Map<Element, HTMLIFrameElement>();
    document
      .querySelectorAll<HTMLIFrameElement>('iframe[sandbox]')
      .forEach((iframe) => {
        if (
          !isIsolatedIframe(iframe) ||
          iframe.closest(ASSISTANT_CHROME_SELECTOR)
        ) {
          return;
        }
        const rect = iframe.getBoundingClientRect();
        const overlay = document.createElement('div');
        overlay.dataset.testid =
          'assistant-page-reference-isolated-frame-overlay';
        Object.assign(overlay.style, {
          cursor: 'not-allowed',
          height: `${rect.height}px`,
          left: `${rect.left}px`,
          position: 'fixed',
          top: `${rect.top}px`,
          width: `${rect.width}px`,
          zIndex: '2147483599'
        });
        isolatedFrameOverlays.set(overlay, iframe);
        document.body.append(overlay);
      });

    const selectedElement = (event: MouseEvent) => {
      const pathElements = event
        .composedPath()
        .filter((node): node is Element => node instanceof Element);
      if (
        pathElements.some((element) =>
          element.matches(ASSISTANT_CHROME_SELECTOR)
        )
      ) {
        return null;
      }
      const deepestElement = pathElements[0];
      return (
        isolatedFrameOverlays.get(deepestElement) ?? deepestElement ?? null
      );
    };
    const handleMove = (event: MouseEvent) => {
      const element = selectedElement(event);
      if (!element || isIsolatedIframe(element)) {
        outline.hidden = true;
        return;
      }
      const rect = element.getBoundingClientRect();
      outline.hidden = false;
      Object.assign(outline.style, {
        height: `${rect.height}px`,
        left: `${rect.left}px`,
        top: `${rect.top}px`,
        width: `${rect.width}px`
      });
    };
    const handleClick = (event: MouseEvent) => {
      const element = selectedElement(event);
      if (!element) return;
      event.preventDefault();
      event.stopPropagation();
      if (isIsolatedIframe(element)) {
        setError(unsupportedIsolatedFrameMessage);
        setSelecting(false);
        return;
      }
      const outerHtml = element.outerHTML;
      const actualBytes = byteLength(outerHtml);
      if (actualBytes > maxBytes) {
        setError(tooLargeMessage(actualBytes, maxBytes));
        setSelecting(false);
        return;
      }
      setReference({
        page_url: window.location.href,
        page_title: document.title,
        outer_html: outerHtml
      });
      setSelecting(false);
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        setSelecting(false);
      }
    };

    document.addEventListener('mousemove', handleMove, true);
    document.addEventListener('click', handleClick, true);
    document.addEventListener('keydown', handleKeyDown, true);
    return () => {
      document.removeEventListener('mousemove', handleMove, true);
      document.removeEventListener('click', handleClick, true);
      document.removeEventListener('keydown', handleKeyDown, true);
      outline.remove();
      hint.remove();
      isolatedFrameOverlays.forEach((_iframe, overlay) => overlay.remove());
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
    };
  }, [
    maxBytes,
    selecting,
    selectionHint,
    tooLargeMessage,
    unsupportedIsolatedFrameMessage
  ]);

  return {
    cancelSelection,
    clearReference,
    error,
    reference,
    selecting,
    startSelection
  };
}
