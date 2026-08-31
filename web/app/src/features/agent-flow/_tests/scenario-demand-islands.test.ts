import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

async function source(relativePath: string) {
  return readFile(path.resolve(process.cwd(), relativePath), 'utf8');
}

describe('BGP scenario demand islands', () => {
  test('preloads only the authenticated shell and current application route', async () => {
    const bootstrap = await source('src/app/ApplicationRuntimeBootstrap.tsx');
    const router = await source('src/app/router.tsx');
    const helpMenu = await source('src/app-shell/HelpChromeMenu.tsx');

    expect(bootstrap).toContain("import('../app-shell/AppShellFrame')");
    expect(router).toContain(
      '/^\\/applications\\/[^/]+(?:\\/|$)/u.test(window.location.pathname)'
    );
    expect(router).toContain('void loadApplicationDetailPage()');
    expect(helpMenu).toContain('enabled: helpOpen');
  });

  test('keeps hidden workflow surfaces behind dynamic imports', async () => {
    const frame = await source(
      'src/features/agent-flow/components/editor/AgentFlowCanvasFrame.tsx'
    );
    expect(frame).toContain("import('../debug-console/AgentFlowDebugConsole')");
    expect(frame).toContain("import('../detail/NodeDetailPanel')");
    expect(frame).toContain("import('../history/VersionHistoryPanel')");
    expect(frame).not.toContain(
      "import { AgentFlowDebugConsole } from '../debug-console/AgentFlowDebugConsole'"
    );

    const jsonPreview = await source(
      'src/shared/ui/json-preview/JsonPreviewBlock.tsx'
    );
    expect(jsonPreview).toContain("import('../../code-block/monaco-runtime')");
    expect(jsonPreview).not.toContain(
      "import { loadMonacoEditorModule } from '../../code-block/monaco-runtime'"
    );
  });

  test('stages assistant shell, conversation and activity', async () => {
    const trigger = await source(
      'src/features/agent-flow/components/embedded-assistant/EmbeddedAgentAssistant.tsx'
    );
    const preview = await source(
      'src/features/agent-flow/components/embedded-assistant/EmbeddedAgentAssistantPreview.tsx'
    );
    expect(trigger).toContain('embedded-agent-assistant-window-shell');
    expect(preview).toContain("import('./AssistantRunActivityPanel')");
    expect(preview).toContain(
      "import('../debug-console/AgentFlowDebugConsole')"
    );
  });
});
