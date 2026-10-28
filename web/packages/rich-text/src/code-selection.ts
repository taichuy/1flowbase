function codeAt(node: Node | null): HTMLElement | null {
  const element =
    node?.nodeType === 1 ? (node as Element) : node?.parentElement;
  return (
    element?.closest<HTMLElement>('.vditor-ir [data-type="code-block"] code') ??
    null
  );
}

function textOffset(root: Node, node: Node, offset: number) {
  const range = root.ownerDocument!.createRange();
  range.selectNodeContents(root);
  range.setEnd(node, offset);
  return range.toString().length;
}

function textPoint(root: Node, offset: number): [Node, number] {
  const walker = root.ownerDocument!.createTreeWalker(root, 4 /* SHOW_TEXT */);
  let node = walker.nextNode();
  while (node) {
    const length = node.textContent?.length ?? 0;
    if (offset <= length) return [node, offset];
    offset -= length;
    node = walker.nextNode();
  }
  return [root, root.childNodes.length];
}

/** Preserve an IR preview selection when Vditor's click handler opens its source. */
export function preserveCodeSelection(mount: HTMLElement) {
  const document = mount.ownerDocument;
  let pending: {
    source: HTMLElement;
    anchor: number;
    focus: number;
    scroll: HTMLElement;
    top: number;
    left: number;
  } | null = null;
  const captureClick = () => {
    pending = null;
    const selection = document.getSelection();
    if (!selection || selection.isCollapsed) return;
    const code = codeAt(selection.anchorNode);
    if (!code || code !== codeAt(selection.focusNode)) return;
    const preview = code.closest('.vditor-ir__preview');
    const source =
      preview?.previousElementSibling?.querySelector<HTMLElement>('code');
    const scroll = code.closest<HTMLElement>('pre[contenteditable="true"]');
    // Only map actual code previews, never rendered diagrams or unrelated blocks.
    if (!source || !scroll || code.textContent !== source.textContent) return;
    pending = {
      source,
      anchor: textOffset(code, selection.anchorNode!, selection.anchorOffset),
      focus: textOffset(code, selection.focusNode!, selection.focusOffset),
      scroll,
      top: scroll.scrollTop,
      left: scroll.scrollLeft
    };
  };
  const restoreClick = () => {
    const saved = pending;
    pending = null;
    if (!saved?.source.isConnected) return;
    document
      .getSelection()
      ?.setBaseAndExtent(
        ...textPoint(saved.source, saved.anchor),
        ...textPoint(saved.source, saved.focus)
      );
    saved.scroll.scrollTop = saved.top;
    saved.scroll.scrollLeft = saved.left;
  };
  const copyKey = (event: KeyboardEvent) => {
    if (
      !(event.ctrlKey || event.metaKey) ||
      event.altKey ||
      event.shiftKey ||
      event.key.toLowerCase() !== 'c' ||
      event.isComposing
    )
      return;
    const selection = document.getSelection();
    const code = codeAt(selection?.anchorNode ?? null);
    if (
      selection &&
      !selection.isCollapsed &&
      code &&
      code === codeAt(selection.focusNode)
    ) {
      // Keep the browser's copy event, but avoid Vditor recording an undo caret for C.
      event.stopPropagation();
    }
  };
  mount.addEventListener('click', captureClick, true);
  mount.addEventListener('click', restoreClick);
  mount.addEventListener('keydown', copyKey, true);
  return () => {
    mount.removeEventListener('click', captureClick, true);
    mount.removeEventListener('click', restoreClick);
    mount.removeEventListener('keydown', copyKey, true);
  };
}
