import fs from 'node:fs';
import path from 'node:path';

import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { NodePickerPopover } from '../components/node-picker/NodePickerPopover';
import { calculateNodePickerMaxHeight } from '../components/node-picker/node-picker-layout';
import {
  buildNodePickerOptions,
  type NodePickerOption
} from '../lib/plugin-node-definitions';
import '../../workflow/register';
import {
  createBuiltinCatalogNode,
  createNodeFieldContract,
  createPluginCatalogNode,
  createPluginNodeIdentity
} from './fixtures/application-node-catalog';

const agentBuiltinOptions = buildNodePickerOptions([
  createBuiltinCatalogNode('start', {
    title: 'Start',
    category: 'io'
  }),
  createBuiltinCatalogNode('llm', {
    title: 'LLM',
    category: 'generation',
    field_contract: createNodeFieldContract({
      input_fields: [
        {
          key: 'bindings.prompt_messages',
          required: true,
          value_types: ['prompt_messages'],
          allowed_values: []
        }
      ]
    })
  }),
  createBuiltinCatalogNode('if_else', {
    title: 'If / Else',
    category: 'control'
  }),
  createBuiltinCatalogNode('variable_assigner', {
    title: 'Variable Assigner',
    category: 'data'
  }),
  createBuiltinCatalogNode('http_request', {
    title: 'HTTP Request',
    category: 'external'
  })
]);

const workflowBuiltinOptions = buildNodePickerOptions([
  createBuiltinCatalogNode('workflow_start', {
    title: 'Workflow Start',
    category: 'io'
  }),
  createBuiltinCatalogNode('workflow_end', {
    title: 'Workflow End',
    category: 'io'
  }),
  createBuiltinCatalogNode('llm', {
    title: 'LLM',
    category: 'generation'
  }),
  createBuiltinCatalogNode('code', {
    title: 'Code',
    category: 'data'
  })
]);

const readyPluginNode = createPluginCatalogNode();
const unavailablePluginNode = createPluginCatalogNode(
  createPluginNodeIdentity({
    installation_id: 'installation-2',
    provider_code: 'sql_pack',
    plugin_unique_identifier: 'sql_pack',
    package_id: 'sql_pack@0.1.0',
    plugin_id: 'sql_pack@0.1.0',
    contribution_code: 'sql_exporter',
    title: 'SQL Exporter',
    description: 'Export rows to sql'
  }),
  {
    title: 'SQL Exporter',
    runtime_status: 'unavailable',
    dependency_status: 'missing_plugin'
  }
);
const pluginOptions: NodePickerOption[] = buildNodePickerOptions([
  readyPluginNode,
  unavailablePluginNode
]);

describe('NodePickerPopover', () => {
  test('groups built-in nodes by workflow purpose', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 LLM 后新增节点"
        open
        options={agentBuiltinOptions}
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
      screen.getByRole('menuitem', { name: /Variable Assigner/i })
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
        options={agentBuiltinOptions}
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

  test('AC-002 searches contract field keys without description rows', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 LLM 后新增节点"
        open
        options={agentBuiltinOptions}
        onOpenChange={vi.fn()}
        onPickNode={vi.fn()}
      />
    );

    fireEvent.change(screen.getByRole('textbox', { name: '搜索节点' }), {
      target: { value: 'bindings.prompt_messages' }
    });

    expect(screen.getByRole('menuitem', { name: 'LLM' })).toBeInTheDocument();
    expect(
      document.querySelector('.agent-flow-node-picker__description')
    ).not.toBeInTheDocument();
  });

  test('AC-002 excludes hidden nodes while keeping published unavailable nodes disabled', () => {
    const options = buildNodePickerOptions([
      createBuiltinCatalogNode('knowledge_retrieval', {
        title: 'Knowledge Retrieval',
        category: 'generation',
        authoring_status: 'hidden',
        runtime_status: 'unavailable'
      }),
      createBuiltinCatalogNode('llm', {
        title: 'LLM',
        category: 'generation',
        runtime_status: 'unavailable'
      })
    ]);

    render(
      <NodePickerPopover
        ariaLabel="在 LLM 后新增节点"
        open
        options={options}
        onOpenChange={vi.fn()}
        onPickNode={vi.fn()}
      />
    );

    expect(
      screen.queryByRole('menuitem', { name: 'Knowledge Retrieval' })
    ).not.toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'LLM' })).toBeDisabled();
    expect(
      document.querySelector('.agent-flow-node-picker__description')
    ).not.toBeInTheDocument();
  });

  test('renders workflow picker options with general execution nodes', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 Workflow 后新增节点"
        open
        options={workflowBuiltinOptions}
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
    expect(
      screen.getByRole('menuitem', { name: /^LLM$/i })
    ).toBeInTheDocument();
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

  test('preserves unified catalog order and nested plugin identity', () => {
    const options = buildNodePickerOptions([
      createBuiltinCatalogNode('workflow_start', {
        title: 'Workflow Start',
        category: 'io'
      }),
      readyPluginNode
    ]);

    expect(options[0]).toMatchObject({
      kind: 'builtin',
      type: 'workflow_start'
    });
    expect(options.at(-1)).toMatchObject({
      kind: 'plugin_contribution',
      label: 'OpenAI Prompt',
      disabled: false,
      plugin: {
        contribution_code: 'openai_prompt'
      }
    });
  });

  test('keeps the default picker empty without server catalog options', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 LLM 后新增节点"
        open
        onOpenChange={vi.fn()}
        onPickNode={vi.fn()}
      />
    );

    expect(screen.getByText('暂无内置节点')).toBeInTheDocument();
    expect(screen.queryByRole('menuitem')).not.toBeInTheDocument();
  });

  test('keeps category tabs and search above the scrollable node list', () => {
    render(
      <NodePickerPopover
        ariaLabel="在 LLM 后新增节点"
        open
        options={[...agentBuiltinOptions, ...pluginOptions]}
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
    expect(
      document.querySelector('.agent-flow-node-picker__description')
    ).not.toBeInTheDocument();
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
    expect(listBlock).toMatch(
      /scroll-padding-bottom:\s*var\(\s*--agent-flow-node-picker-list-bottom-padding,\s*40px\s*\);/
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
