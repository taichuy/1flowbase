import { useCallback, useLayoutEffect, useState } from 'react';

export function useFrontstagePageCanvasWidth() {
  const [containerNode, setContainerNode] = useState<HTMLDivElement | null>(
    null
  );
  const [width, setWidth] = useState(0);
  const containerRef = useCallback((node: HTMLDivElement | null) => {
    setContainerNode(node);
  }, []);

  useLayoutEffect(() => {
    if (!containerNode) return;

    const updateWidth = (nextWidth: number) => {
      if (!Number.isFinite(nextWidth) || nextWidth <= 0) return;
      setWidth((currentWidth) =>
        Math.abs(currentWidth - nextWidth) < 0.5 ? currentWidth : nextWidth
      );
    };

    updateWidth(containerNode.offsetWidth);
    if (typeof ResizeObserver === 'undefined') return;

    const observer = new ResizeObserver(([entry]) => {
      if (entry) updateWidth(entry.contentRect.width);
    });
    observer.observe(containerNode);
    return () => observer.disconnect();
  }, [containerNode]);

  return { width, containerNode, containerRef };
}
