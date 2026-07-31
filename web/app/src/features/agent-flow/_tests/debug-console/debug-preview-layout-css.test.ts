import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { describe, expect, test } from 'vitest';

const debugMessageCss = readFileSync(
  join(
    process.cwd(),
    'src/features/agent-flow/components/debug-console/conversation/debug-message.css'
  ),
  'utf8'
);
const shellCss = readFileSync(
  join(
    process.cwd(),
    'src/features/agent-flow/components/editor/styles/shell.css'
  ),
  'utf8'
);
const conversationLogCss = readFileSync(
  join(
    process.cwd(),
    'src/features/agent-flow/components/debug-console/conversation-log-panel.css'
  ),
  'utf8'
);

function cssBlock(css: string, selector: string) {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = css.match(new RegExp(`${escapedSelector}\\s*\\{([^}]*)\\}`));

  return match?.[1] ?? '';
}

describe('debug preview responsive layout CSS', () => {
  const layoutPropertiesWithPixelValues =
    /(?:gap|padding(?:-[a-z]+)?|margin(?:-[a-z]+)?|width|height|min-width|min-height|max-width|max-height|inline-size|block-size|min-inline-size|min-block-size|border-radius|grid-template-columns):[^;]*\d+px/;

  test('keeps preview internals free of fixed pixel layout values', () => {
    [
      '.agent-flow-editor__debug-console',
      '.agent-flow-editor__debug-console-header',
      '.agent-flow-editor__debug-messages',
      '.agent-flow-editor__debug-message',
      '.agent-flow-editor__debug-message-main',
      '.agent-flow-editor__debug-message--assistant .agent-flow-editor__debug-message-main',
      '.agent-flow-editor__debug-message--user .agent-flow-editor__debug-message-main',
      '.agent-flow-editor__debug-message--user .agent-flow-editor__debug-message-content.ant-typography',
      '.agent-flow-editor__debug-composer',
      '.agent-flow-editor__debug-composer-box',
      '.agent-flow-editor__debug-composer-submit'
    ].forEach((selector) => {
      expect(cssBlock(shellCss, selector), selector).not.toMatch(
        layoutPropertiesWithPixelValues
      );
    });

    [
      '.agent-flow-editor__debug-workflow-process',
      '.agent-flow-editor__debug-workflow-header',
      '.agent-flow-editor__debug-workflow-title',
      '.agent-flow-editor__debug-workflow-list',
      '.agent-flow-editor__debug-workflow-row',
      '.agent-flow-editor__debug-workflow-node-icon',
      '.agent-flow-editor__debug-workflow-node-main'
    ].forEach((selector) => {
      expect(cssBlock(debugMessageCss, selector), selector).not.toMatch(
        layoutPropertiesWithPixelValues
      );
    });
  });

  test('lets the preview shell expand inside the shared dock container', () => {
    expect(cssBlock(shellCss, '.agent-flow-editor__dock-panel')).toMatch(
      /flex:\s*1\b/
    );
  });

  test('opens the conversation log through the shared side dock shell', () => {
    const debugConsoleDock = cssBlock(
      shellCss,
      '.agent-flow-editor__debug-console-dock'
    );
    const conversationLogDock = cssBlock(
      shellCss,
      '.agent-flow-editor__conversation-log-dock'
    );
    const conversationLogPanel = cssBlock(
      conversationLogCss,
      '.agent-flow-editor__conversation-log-panel'
    );

    expect(debugConsoleDock).toMatch(/overflow:\s*visible/);
    expect(conversationLogDock).toMatch(/position:\s*absolute/);
    expect(conversationLogDock).toMatch(/overflow:\s*hidden/);
    expect(conversationLogPanel).not.toMatch(/position:\s*absolute/);
    expect(conversationLogPanel).not.toMatch(/flex:\s*1 1 48%/);
  });

  test('does not lock preview rows or composer controls to fixed pixel columns', () => {
    expect(
      cssBlock(debugMessageCss, '.agent-flow-editor__debug-workflow-row')
    ).not.toMatch(/grid-template-columns:[^;]*\d+px/);
    expect(
      cssBlock(shellCss, '.agent-flow-editor__debug-composer-box')
    ).not.toMatch(/grid-template-columns:[^;]*\d+px/);
  });

  test('keeps workflow node title and metric on one row', () => {
    const nodeMain = cssBlock(
      debugMessageCss,
      '.agent-flow-editor__debug-workflow-node-main'
    );

    expect(nodeMain).toMatch(/display:\s*inline-flex/);
    expect(nodeMain).toMatch(/align-items:\s*center/);
    expect(nodeMain).not.toMatch(/display:\s*grid/);
  });

  test('keeps icon and submit affordances relative to text scale', () => {
    expect(
      cssBlock(debugMessageCss, '.agent-flow-editor__debug-workflow-node-icon')
    ).not.toMatch(/(?:width|height):\s*\d+px/);
    expect(
      cssBlock(shellCss, '.agent-flow-editor__debug-composer-submit')
    ).not.toMatch(/(?:width|height|min-width):\s*\d+px/);
  });
});
