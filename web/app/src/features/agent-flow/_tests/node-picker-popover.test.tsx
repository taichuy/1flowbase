import fs from 'node:fs';
import path from 'node:path';

import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { NodePickerPopover } from '../components/node-picker/NodePickerPopover';
import { calculateNodePickerMaxHeight } from '../components/node-picker/node-picker-layout';
import {
  BUILTIN_NODE_PICKER_OPTIONS,
  type NodePickerOption
} from '../lib/plugin-node-definitions';
import {
  WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS,
  buildWorkflowNodePickerOptions
} from '../../workflow/lib/picker-options';

const pluginOptions: NodePickerOption[] = [
  {
    kind: 'plugin_contribution',
    label: 'OpenAI Prompt',
    disabled: false,
    disabledReason: null,
    contribution: {
      installation_id: 'installation-1',
      provider_code: 'prompt_pack',
      plugin_id: 'prompt_pack@0.1.0',
      plugin_version: '0.1.0',
      contribution_code: 'openai_prompt',
      node_shell: 'action',
      plugin_unique_identifier: 'prompt_pack',
      package_id: 'prompt_pack@0.1.0',
      contribution_checksum: 'sha256:openai-prompt',
      compiled_contribution_hash: 'sha256:compiled-openai-prompt',
      category: 'generation',
      title: 'OpenAI Prompt',
      description: 'Generate prompt output',
      dependency_status: 'ready',
      schema_version: '1flowbase.node-contribution/v2',
      output_schema_snapshot: {
        outputs: [{ key: 'answer', title: 'Answer', valueType: 'string' }]
      },
      experimental: false,
      icon: 'sparkles',
      schema_ui: {},
      output_schema: {
        outputs: [{ key: 'answer', title: 'Answer', valueType: 'string' }]
      },
      side_effect_policy: 'external_read',
      infra_contracts: [],
      required_auth: [],
      visibility: 'public',
      dependency_installation_kind: 'model_provider',
      dependency_plugin_version_range: '^0.1.0'
    }
  },
  {
    kind: 'plugin_contribution',
    label: 'SQL Exporter',
    disabled: true,
    disabledReason: '缺少依赖插件',
    contribution: {
      installation_id: 'installation-2',
      provider_code: 'sql_pack',
      plugin_id: 'sql_pack@0.1.0',
      plugin_version: '0.1.0',
      contribution_code: 'sql_exporter',
      node_shell: 'action',
      plugin_unique_identifier: 'sql_pack',
      package_id: 'sql_pack@0.1.0',
      contribution_checksum: 'sha256:sql-exporter',
      compiled_contribution_hash: 'sha256:compiled-sql-exporter',
      category: 'export',
      title: 'SQL Exporter',
      description: 'Export rows to sql',
      dependency_status: 'missing_plugin',
      schema_version: '1flowbase.node-contribution/v2',
      output_schema_snapshot: {
        outputs: [{ key: 'result', title: 'Result', valueType: 'json' }]
      },
      experimental: false,
      icon: 'database',
      schema_ui: {},
      output_schema: {
        outputs: [{ key: 'result', title: 'Result', valueType: 'json' }]
      },
      side_effect_policy: 'external_read',
      infra_contracts: [],
      required_auth: [],
      visibility: 'public',
      dependency_installation_kind: 'model_provider',
      dependency_plugin_version_range: '^0.1.0'
    }
  }
];

describe('NodePickerPopover', () => {
  test('groups built-in nodes by workflow purpose', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 LLM 后新增节点"
        open
        onOpenChange={vi.fn()}
        onPickNode={vi.fn()}
      />
    );

    expect(screen.getByText('起止输出')).toBeInTheDocument();
    expect(screen.getByText('模型与生成')).toBeInTheDocument();
    expect(screen.getByText('流程控制')).toBeInTheDocument();
    expect(screen.getByText('数据处理')).toBeInTheDocument();
    expect(screen.getByText('外部能力')).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: /LLM/i })).toBeInTheDocument();
    expect(
      screen.getByRole('menuitem', { name: /Knowledge Retrieval/i })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('menuitem', { name: /变量赋值/i })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: /Parameter Extractor/i })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: /Human Input/i })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: /Iteration/i })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: /Loop/i })
    ).not.toBeInTheDocument();
  });

  test('filters node groups through the picker search', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 LLM 后新增节点"
        open
        onOpenChange={vi.fn()}
        onPickNode={vi.fn()}
      />
    );

    fireEvent.change(screen.getByRole('textbox', { name: '搜索节点' }), {
      target: { value: 'http' }
    });

    expect(
      screen.getByRole('menuitem', { name: /HTTP Request/i })
    ).toBeInTheDocument();
    expect(screen.getByText('外部能力')).toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: /LLM/i })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('模型与生成')).not.toBeInTheDocument();
  });

  test('renders workflow picker options with general execution nodes', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 Workflow 后新增节点"
        open
        options={WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS}
        onOpenChange={vi.fn()}
        onPickNode={vi.fn()}
      />
    );

    expect(
      screen.getByRole('menuitem', { name: /Workflow Start/i })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('menuitem', { name: /Workflow End/i })
    ).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: /^LLM$/i })).toBeInTheDocument();
    expect(
      screen.getByRole('menuitem', { name: /^Code$/i })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: /^Start$/i })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: /^Answer$/i })
    ).not.toBeInTheDocument();
  });

  test('appends plugin contributions after workflow builtin options', () => {
    const [first] = pluginOptions;

    if (first.kind !== 'plugin_contribution') {
      throw new Error('fixture must be a plugin contribution');
    }

    const options = buildWorkflowNodePickerOptions([first.contribution]);

    expect(
      options.slice(0, WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS.length)
    ).toEqual(WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS);
    expect(options.at(-1)).toMatchObject({
      kind: 'plugin_contribution',
      label: 'OpenAI Prompt',
      disabled: false
    });
  });

  test('keeps category tabs and search above the scrollable node list', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 LLM 后新增节点"
        open
        options={[...BUILTIN_NODE_PICKER_OPTIONS, ...pluginOptions]}
        onOpenChange={vi.fn()}
        onPickNode={vi.fn()}
      />
    );

    expect(screen.getByRole('tab', { name: '内置' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(screen.getByRole('tab', { name: '扩展' })).toHaveAttribute(
      'aria-selected',
      'false'
    );
    expect(
      screen.queryByRole('menuitem', { name: /OpenAI Prompt/i })
    ).not.toBeInTheDocument();

    const searchInput = screen.getByRole('textbox', { name: '搜索节点' });
    const nodeList = screen.getByRole('menu');

    expect(
      screen.getByRole('tablist', { name: '节点来源' })
    ).toBeInTheDocument();
    expect(searchInput).toBeInTheDocument();
    expect(
      within(nodeList).queryByRole('textbox', { name: '搜索节点' })
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: '扩展' }));

    expect(screen.getByRole('tab', { name: '内置' })).toHaveAttribute(
      'aria-selected',
      'false'
    );
    expect(screen.getByRole('tab', { name: '扩展' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(
      screen.getByRole('menuitem', { name: /OpenAI Prompt/i })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: /LLM/i })
    ).not.toBeInTheDocument();
  });

  test('lets mousedown bubble so the surrounding handle can start a connection drag', () => {
    const handleMouseDown = vi.fn();

    render(
      <div onMouseDown={handleMouseDown}>
        <NodePickerPopover
          ariaLabel="在 LLM 后新增节点"
          open={false}
          onOpenChange={vi.fn()}
          onPickNode={vi.fn()}
        />
      </div>
    );

    fireEvent.mouseDown(
      screen.getByRole('button', { name: '在 LLM 后新增节点' })
    );

    expect(handleMouseDown).toHaveBeenCalledTimes(1);
  });

  test('keeps click from bubbling to the surrounding node card', () => {
    const handleClick = vi.fn();

    render(
      <div onClick={handleClick}>
        <NodePickerPopover
          ariaLabel="在 LLM 后新增节点"
          open={false}
          onOpenChange={vi.fn()}
          onPickNode={vi.fn()}
        />
      </div>
    );

    fireEvent.click(screen.getByRole('button', { name: '在 LLM 后新增节点' }));

    expect(handleClick).not.toHaveBeenCalled();
  });

  test('renders plugin contribution entries and disables missing dependencies', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 LLM 后新增节点"
        open
        options={pluginOptions}
        onOpenChange={vi.fn()}
        onPickNode={vi.fn()}
      />
    );

    expect(
      screen.getByRole('menuitem', { name: /OpenAI Prompt/i })
    ).toBeEnabled();
    expect(
      screen.getByRole('menuitem', { name: /SQL Exporter/i })
    ).toBeDisabled();
  });

  test('keeps final picker items clear of the clipped popup edge', () => {
    const canvasControlsCss = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../components/editor/styles/canvas-controls.css'
      ),
      'utf8'
    );
    const listBlock = canvasControlsCss.match(
      /\.agent-flow-node-picker__list\s*\{[\s\S]*?\n\}/
    )?.[0];

    expect(listBlock).toContain(
      'padding-bottom: var(--agent-flow-node-picker-list-bottom-padding, 40px);'
    );
    expect(listBlock).toContain(
      'scroll-padding-bottom: var(--agent-flow-node-picker-list-bottom-padding, 40px);'
    );
  });

  test('sets picker height from the canvas bottom control boundary', async () => {
    const getRectSpy = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function (this: HTMLElement) {
        const baseRect = {
          x: 0,
          y: 0,
          width: 0,
          height: 0,
          top: 0,
          right: 0,
          bottom: 0,
          left: 0,
          toJSON: () => ({})
        };

        if (this.classList.contains('agent-flow-canvas')) {
          return { ...baseRect, bottom: 900 };
        }

        if (
          this.classList.contains('agent-flow-editor__variable-cache-trigger')
        ) {
          return { ...baseRect, bottom: 760 };
        }

        if (this.getAttribute('aria-label') === '在 LLM 后新增节点') {
          return { ...baseRect, top: 260, bottom: 300 };
        }

        return baseRect;
      });

    try {
      render(
        <div className="agent-flow-editor__body">
          <div className="agent-flow-canvas" data-testid="node-picker-canvas">
            <NodePickerPopover
              ariaLabel="在 LLM 后新增节点"
              open
              placement="bottom"
              onOpenChange={vi.fn()}
              onPickNode={vi.fn()}
            />
          </div>
          <button
            className="agent-flow-editor__variable-cache-trigger"
            type="button"
          >
            查看缓存
          </button>
        </div>
      );

      expect(await screen.findByRole('menu')).toBeInTheDocument();
      expect(screen.getByTestId('node-picker-canvas')).toHaveStyle(
        '--agent-flow-node-picker-max-height: 450px'
      );
    } finally {
      getRectSpy.mockRestore();
    }
  });

  test('calculates picker height with a 10px canvas bottom gap', () => {
    expect(
      calculateNodePickerMaxHeight({ canvasBottom: 500, anchorY: 360 })
    ).toBe(130);
    expect(
      calculateNodePickerMaxHeight({ canvasBottom: 500, anchorY: 460 })
    ).toBe(120);
  });

  test('caps picker height at the canvas bottom control boundary', () => {
    expect(
      calculateNodePickerMaxHeight({
        canvasBottom: 900,
        anchorY: 260,
        bottomBoundary: 760
      })
    ).toBe(490);
  });
});
