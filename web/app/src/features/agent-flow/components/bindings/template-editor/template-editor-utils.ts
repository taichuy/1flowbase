import type { EditorState } from 'lexical';

import {
  $getRoot,
  $getSelection,
  $isRangeSelection,
  TextNode,
} from 'lexical';

export function textToEditorState(text: string) {
  const paragraphs = typeof text === 'string' && text.length > 0 ? text.split('\n') : [''];

  return JSON.stringify({
    root: {
      children: paragraphs.map((paragraph) => ({
        children: paragraph.length > 0
          ? [
              {
                detail: 0,
                format: 0,
                mode: 'normal',
                style: '',
                text: paragraph,
                type: 'text',
                version: 1
              }
            ]
          : [],
        direction: null,
        format: '',
        indent: 0,
        type: 'paragraph',
        version: 1
      })),
      direction: null,
      format: '',
      indent: 0,
      type: 'root',
      version: 1
    }
  });
}

export function editorStateToText(editorState: EditorState) {
  return editorState.read(() =>
    $getRoot()
      .getChildren()
      .map((child) => child.getTextContent())
      .join('\n')
  );
}

function getCollapsedTextSelection() {
  const selection = $getSelection();

  if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
    return null;
  }

  if (selection.anchor.type !== 'text') {
    return null;
  }

  const anchorNode = selection.anchor.getNode();

  if (!(anchorNode instanceof TextNode) || !anchorNode.isSimpleText()) {
    return null;
  }

  return {
    selection,
    anchorNode,
    offset: selection.anchor.offset
  };
}

export function getTriggerContext(triggers: ReadonlySet<string>) {
  const context = getCollapsedTextSelection();

  if (!context || context.offset <= 0) {
    return null;
  }

  const text = context.anchorNode.getTextContent();
  const prefix = text.slice(0, context.offset);
  const triggerMatches = Array.from(triggers)
    .map((trigger) => ({
      trigger,
      index: prefix.lastIndexOf(trigger)
    }))
    .filter((match) => match.index >= 0);

  if (triggerMatches.length === 0) {
    return null;
  }

  const latestTrigger = triggerMatches.reduce((latestMatch, match) => {
    if (!latestMatch || match.index > latestMatch.index) {
      return match;
    }

    return latestMatch;
  }, null as { trigger: string; index: number } | null);

  if (!latestTrigger) {
    return null;
  }

  const query = prefix.slice(latestTrigger.index + latestTrigger.trigger.length);

  // 触发符之后出现空白说明用户已经继续正常输入，此时不再把它当变量搜索。
  if (/\s/.test(query)) {
    return null;
  }

  return {
    ...context,
    trigger: latestTrigger.trigger,
    triggerOffset: latestTrigger.index,
    query
  };
}

export function removeTriggerQueryBeforeSelection(
  triggers: ReadonlySet<string>
) {
  const context = getTriggerContext(triggers);

  if (!context) {
    return false;
  }

  if (context.triggerOffset === context.offset) {
    return false;
  }

  if (context.triggerOffset === 0) {
    const [triggerQueryNode] = context.anchorNode.splitText(context.offset);
    triggerQueryNode.remove();
    return true;
  }

  const [, triggerQueryNode] = context.anchorNode.splitText(
    context.triggerOffset,
    context.offset
  );
  triggerQueryNode.remove();
  return true;
}

export function removeTriggerQueryAtDocumentEnd(triggers: ReadonlySet<string>) {
  const lastNode = $getRoot().getLastDescendant();

  if (!(lastNode instanceof TextNode) || !lastNode.isSimpleText()) {
    return false;
  }

  const text = lastNode.getTextContent();
  const triggerMatches = Array.from(triggers)
    .map((trigger) => ({
      trigger,
      index: text.lastIndexOf(trigger)
    }))
    .filter((match) => match.index >= 0);

  if (triggerMatches.length === 0) {
    return false;
  }

  const latestTrigger = triggerMatches.reduce((latestMatch, match) => {
    if (!latestMatch || match.index > latestMatch.index) {
      return match;
    }

    return latestMatch;
  }, null as { trigger: string; index: number } | null);

  if (!latestTrigger) {
    return false;
  }

  const query = text.slice(latestTrigger.index + latestTrigger.trigger.length);

  if (/\s/.test(query)) {
    return false;
  }

  if (latestTrigger.index === 0) {
    const parent = lastNode.getParent();
    const textNodes = $getRoot().getAllTextNodes();
    const lastNodeIndex = textNodes.findIndex(
      (candidate) => candidate.getKey() === lastNode.getKey()
    );
    const previousTextNode =
      lastNodeIndex > 0
        ? textNodes
            .slice(0, lastNodeIndex)
            .reverse()
            .find((candidate) => candidate.isSimpleText())
        : null;

    lastNode.remove();

    if (
      parent &&
      parent !== $getRoot() &&
      parent.getChildrenSize() === 0
    ) {
      parent.remove();
    }

    if (previousTextNode) {
      previousTextNode.selectEnd();
    } else {
      $getRoot().selectEnd();
    }

    return true;
  }

  const [textBeforeTrigger, triggerQueryNode] = lastNode.splitText(
    latestTrigger.index
  );
  triggerQueryNode.remove();
  textBeforeTrigger.selectEnd();
  return true;
}
