import { useLayoutEffect, useState } from 'react';

/** Allocate the remaining page viewport, independent of the block's content. */
export function useFrontstageCanvasViewportHeight(
  canvas: HTMLElement | null,
  enabled: boolean,
  isDesignMode = false
) {
  const [height, setHeight] = useState(0);

  useLayoutEffect(() => {
    const owner = canvas?.closest<HTMLElement>(
      '[data-flowbase-frontstage-scroll-owner]'
    );
    const view = canvas?.ownerDocument.defaultView;
    if (!enabled || !canvas || !owner || !view) {
      setHeight(0);
      return;
    }

    const ancestors: HTMLElement[] = [];
    const footer = owner.querySelector<HTMLElement>(
      '[data-flowbase-frontstage-canvas-footer]'
    );
    for (
      let node: HTMLElement | null = owner;
      node;
      node = node.parentElement
    ) {
      ancestors.push(node);
    }
    const measure = () => {
      const bounds = canvas.getBoundingClientRect();
      // Retained pages can be display:none; keep their last allocation.
      if (bounds.width <= 0) return;
      let bottomInsets = 0;
      if (footer) {
        const style = view.getComputedStyle(footer);
        bottomInsets +=
          footer.getBoundingClientRect().height +
          (Number.parseFloat(style.marginTop) || 0) +
          (Number.parseFloat(style.marginBottom) || 0);
      }
      let scrollTop = view.scrollY;
      for (const node of ancestors) {
        const style = view.getComputedStyle(node);
        bottomInsets +=
          (Number.parseFloat(style.paddingBottom) || 0) +
          (Number.parseFloat(style.borderBottomWidth) || 0);
        if (node !== canvas.ownerDocument.scrollingElement) {
          scrollTop += node.scrollTop;
        }
      }
      const next = Math.max(
        0,
        Math.floor(view.innerHeight - bounds.top - scrollTop - bottomInsets)
      );
      setHeight((current) => (current === next ? current : next));
    };
    measure();
    const observer = new ResizeObserver(measure);
    for (const node of [canvas, ...ancestors]) observer.observe(node);
    if (footer) observer.observe(footer);
    view.addEventListener('resize', measure);
    return () => {
      observer.disconnect();
      view.removeEventListener('resize', measure);
    };
  }, [canvas, enabled, isDesignMode]);

  return enabled ? height : 0;
}
