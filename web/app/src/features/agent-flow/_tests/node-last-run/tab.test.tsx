import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { AppProviders } from '../../../../app/AppProviders';
import * as runtimeApi from '../../api/runtime';
import { NodeLastRunTab } from '../../components/detail/tabs/NodeLastRunTab';
import { createAgentFlowNodeSchemaAdapter } from '../../schema/node-schema-adapter';
import { resolveAgentFlowNodeSchema } from '../../schema/node-schema-registry';

function createNodeLastRunSchemaProps() {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
  const nodeId = 'node-llm';

  return {
    schema: resolveAgentFlowNodeSchema('llm'),
    adapter: createAgentFlowNodeSchemaAdapter({
      document,
      nodeId,
      setWorkingDocument: vi.fn(),
      dispatch: vi.fn()
    })
  };
}

describe('NodeLastRunTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test('AC-1993-004 uses the shared loading state before the first run value is available', () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockImplementation(
      () => new Promise(() => undefined)
    );

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-llm" />
      </AppProviders>
    );

    expect(
      screen.getByRole('status', {
        name: 'thinking'
      })
    ).toBeVisible();
  });

  test('AC-1993-005 renders an empty state instead of loading after an empty result', async () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue(null);

    const { container } = render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-llm" />
      </AppProviders>
    );

    await waitFor(() => {
      expect(container.querySelector('.ant-empty')).toBeVisible();
    });
    expect(
      screen.queryByRole('status', {
        name: 'thinking'
      })
    ).not.toBeInTheDocument();
  });

  test('renders empty state when the selected node has not run yet', async () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue(null);

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-llm" />
      </AppProviders>
    );

    expect(
      await screen.findByText('当前节点还没有运行记录')
    ).toBeInTheDocument();
    expect(screen.getByText('运行记录')).toBeInTheDocument();
  });

  test('uses the schema last-run template for active-run empty state', async () => {
    vi.spyOn(runtimeApi, 'fetchApplicationRunNodeLastRun').mockResolvedValue(
      null
    );

    render(
      <AppProviders>
        <NodeLastRunTab
          activeRunId="run-active"
          applicationId="app-1"
          nodeId="node-llm"
          {...createNodeLastRunSchemaProps()}
        />
      </AppProviders>
    );

    expect(
      await screen.findByText('当前运行没有该节点记录')
    ).toBeInTheDocument();
    expect(screen.getByText('运行记录')).toBeInTheDocument();
    expect(screen.queryByText('暂无运行输入输出')).not.toBeInTheDocument();
    expect(screen.queryByText('暂无运行元数据')).not.toBeInTheDocument();
  });

  test('renders runtime-backed summary, io and metadata cards', async () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue({
      flow_run: {
        id: 'run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_node_preview',
        status: 'succeeded',
        target_node_id: 'node-llm',
        input_payload: {
          'node-start.query': '总结退款政策'
        },
        output_payload: {
          resolved_inputs: {
            user_prompt: '总结退款政策'
          }
        },
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        created_at: '2026-04-17T09:00:00Z'
      },
      node_run: {
        id: 'node-run-1',
        flow_run_id: 'run-1',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: {
          user_prompt: '总结退款政策'
        },
        output_payload: {
          text: '退款政策摘要',
          reasoning_content: '先分析退款场景',
          provider_metadata: {
            response_id: 'chatcmpl-1'
          },
          provider_route: {
            provider_code: 'openai_compatible'
          }
        },
        error_payload: null,
        metrics_payload: {
          output_contract_count: 1,
          total_tokens: 72,
          provider_instance_id: 'provider-openai-prod',
          provider_code: 'openai_compatible',
          protocol: 'openai_responses',
          finish_reason: 'stop',
          route: 'primary',
          attempt: 2
        },
        debug_payload: {
          assistant_message: {
            role: 'assistant',
            content: '退款政策摘要'
          },
          provider_events: [
            {
              type: 'text_delta',
              delta: '退款政策摘要'
            }
          ]
        },
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      checkpoints: [],
      events: []
    });

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-llm" />
      </AppProviders>
    );

    expect(await screen.findByText('运行摘要')).toBeInTheDocument();
    expect(screen.queryByText('运行模式')).not.toBeInTheDocument();
    expect(screen.queryByText('目标节点')).not.toBeInTheDocument();
    expect(screen.getByText('token')).toBeInTheDocument();
    expect(screen.getByText('耗时(ms)')).toBeInTheDocument();
    expect(screen.getByText('72')).toBeInTheDocument();
    expect(screen.getByLabelText('输入 JSON')).toHaveTextContent('user_prompt');
    expect(screen.getByLabelText('输入 JSON')).toHaveTextContent(
      '总结退款政策'
    );
    const outputJson = screen.getByLabelText('输出 JSON');
    expect(outputJson).toHaveTextContent('退款政策摘要');
    expect(outputJson).toHaveTextContent('reasoning_content');
    expect(outputJson).toHaveTextContent('先分析退款场景');
    expect(outputJson).toHaveTextContent('provider_metadata');
    expect(outputJson).toHaveTextContent('provider_route');
    expect(outputJson).not.toHaveTextContent('provider_events');
    expect(outputJson).not.toHaveTextContent('text_delta');
    const processJson = screen.getByLabelText('数据处理 JSON');
    expect(processJson).toHaveTextContent('provider_events');
    expect(processJson).toHaveTextContent('text_delta');
    expect(processJson).toHaveTextContent('assistant_message');
    expect(processJson).not.toHaveTextContent('provider_route');
    expect(screen.queryByLabelText('指标 JSON')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Debug JSON')).not.toBeInTheDocument();
    expect(screen.queryByText('执行人')).not.toBeInTheDocument();
    expect(screen.queryByText('Compiled Plan')).not.toBeInTheDocument();
    expect(screen.queryByText('输出契约数')).not.toBeInTheDocument();

    const inputToggle = screen.getByRole('button', { name: '输入' });
    expect(inputToggle).toHaveAttribute('aria-expanded', 'true');

    fireEvent.click(inputToggle);

    expect(inputToggle).toHaveAttribute('aria-expanded', 'false');
    expect(
      screen.getByRole('button', { name: '放大查看输入 JSON' })
    ).toBeDisabled();
  });

  test('uses the active run scoped node endpoint instead of latest node run fallback', async () => {
    const fetchScopedNodeRunSpy = vi
      .spyOn(runtimeApi, 'fetchApplicationRunNodeLastRun')
      .mockResolvedValue({
        flow_run: {
          id: 'run-active',
          application_id: 'app-1',
          flow_id: 'flow-1',
          draft_id: 'draft-1',
          compiled_plan_id: 'plan-1',
          run_mode: 'debug_flow_run',
          status: 'succeeded',
          target_node_id: null,
          input_payload: {
            'node-start': { query: '统一 run scope' }
          },
          output_payload: {
            answer: '统一结果'
          },
          error_payload: null,
          created_by: 'user-1',
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          created_at: '2026-04-17T09:00:00Z'
        },
        node_run: {
          id: 'node-run-1',
          flow_run_id: 'run-active',
          node_id: 'node-llm',
          node_type: 'llm',
          node_alias: 'LLM',
          status: 'succeeded',
          input_payload: {
            user_prompt: '统一 run scope'
          },
          output_payload: {
            text: '统一结果'
          },
          error_payload: null,
          metrics_payload: {
            total_tokens: 16
          },
          debug_payload: {},
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z'
        },
        checkpoints: [],
        events: []
      });
    const fetchNodeLastRunSpy = vi.spyOn(runtimeApi, 'fetchNodeLastRun');

    render(
      <AppProviders>
        <NodeLastRunTab
          activeRunId="run-active"
          applicationId="app-1"
          nodeId="node-llm"
        />
      </AppProviders>
    );

    expect(await screen.findByText('运行摘要')).toBeInTheDocument();
    expect(fetchScopedNodeRunSpy).toHaveBeenCalledWith(
      'app-1',
      'run-active',
      'node-llm'
    );
    expect(fetchNodeLastRunSpy).not.toHaveBeenCalled();
    expect(screen.getByLabelText('输出 JSON')).toHaveTextContent('统一结果');
  });

  test('renders API-provided node output without frontend envelope rewriting', async () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue({
      flow_run: {
        id: 'run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_node_preview',
        status: 'succeeded',
        target_node_id: 'node-llm',
        input_payload: {},
        output_payload: {},
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        created_at: '2026-04-17T09:00:00Z'
      },
      node_run: {
        id: 'node-run-1',
        flow_run_id: 'run-1',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: {},
        output_payload: {
          text: '退款政策摘要',
          usage: { total_tokens: 128 }
        },
        error_payload: null,
        metrics_payload: {},
        debug_payload: {},
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      checkpoints: [],
      events: []
    });

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-llm" />
      </AppProviders>
    );

    const outputJson = await screen.findByLabelText('输出 JSON');
    expect(outputJson).toHaveTextContent('退款政策摘要');
    expect(outputJson).toHaveTextContent('total_tokens');
  });

  test('keeps output fields even when debug payload has the same keys', async () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue({
      flow_run: {
        id: 'run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_node_preview',
        status: 'succeeded',
        target_node_id: 'node-llm',
        input_payload: {},
        output_payload: {},
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        created_at: '2026-04-17T09:00:00Z'
      },
      node_run: {
        id: 'node-run-1',
        flow_run_id: 'run-1',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: {},
        output_payload: {
          text: '退款政策摘要',
          provider_events: [{ type: 'text_delta', delta: '退款政策摘要' }]
        },
        error_payload: null,
        metrics_payload: {},
        debug_payload: {
          provider_events: [{ type: 'text_delta', delta: '退款政策摘要' }]
        },
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      checkpoints: [],
      events: []
    });

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-llm" />
      </AppProviders>
    );

    const outputJson = await screen.findByLabelText('输出 JSON');
    expect(outputJson).toHaveTextContent('provider_events');
    expect(outputJson).toHaveTextContent('text_delta');
  });

  test('loads truncated last-run field artifact on explicit action', async () => {
    vi.spyOn(runtimeApi, 'fetchRuntimeDebugArtifacts').mockResolvedValue({
      artifacts: [
        {
          artifact_ref: 'artifact-1',
          content_type: 'application/json',
          value: '完整 Last Run 内容'
        }
      ]
    });
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue({
      flow_run: {
        id: 'run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_node_preview',
        status: 'succeeded',
        target_node_id: 'node-llm',
        input_payload: {},
        output_payload: {},
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        created_at: '2026-04-17T09:00:00Z'
      },
      node_run: {
        id: 'node-run-1',
        flow_run_id: 'run-1',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: {},
        output_payload: {
          text: {
            __runtime_debug_artifact: true,
            artifact_scope: 'field',
            field_path: ['text'],
            is_truncated: true,
            original_size_bytes: 4096,
            preview_size_bytes: 128,
            content_type: 'application/json',
            artifact_ref: 'artifact-1',
            preview: '{"text":"preview'
          },
          usage: { total_tokens: 32 }
        },
        error_payload: null,
        metrics_payload: {},
        debug_payload: {},
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      checkpoints: [],
      events: []
    });

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-llm" />
      </AppProviders>
    );

    fireEvent.click(await screen.findByRole('button', { name: '加载完整值' }));

    expect(runtimeApi.fetchRuntimeDebugArtifacts).toHaveBeenCalledWith(
      'app-1',
      ['artifact-1']
    );
    expect(await screen.findByLabelText('输出 JSON')).toHaveTextContent(
      '完整 Last Run 内容'
    );
    expect(await screen.findByLabelText('输出 JSON')).toHaveTextContent(
      'total_tokens'
    );
  });

  test('loads start input field artifacts back into their original fields', async () => {
    vi.spyOn(runtimeApi, 'fetchRuntimeDebugArtifacts').mockResolvedValue({
      artifacts: [
        {
          artifact_ref: 'artifact-start-history',
          content_type: 'application/json',
          value: [{ role: 'user', content: '旧问题' }]
        },
        {
          artifact_ref: 'artifact-start-tools',
          content_type: 'application/json',
          value: [{ name: 'read_file' }]
        }
      ]
    });
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue({
      flow_run: {
        id: 'run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_node_preview',
        status: 'succeeded',
        target_node_id: 'node-start',
        input_payload: {},
        output_payload: {},
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        created_at: '2026-04-17T09:00:00Z'
      },
      node_run: {
        id: 'node-run-1',
        flow_run_id: 'run-1',
        node_id: 'node-start',
        node_type: 'start',
        node_alias: 'Start',
        status: 'succeeded',
        input_payload: {
          query: '总结退款政策',
          model: 'deepseek-chat',
          files: [{ name: 'refund.md' }],
          sys: { workflow_run_id: 'run-1' },
          env: { ApiBaseUrl: 'https://api.example.com' },
          history: {
            __runtime_debug_artifact: true,
            artifact_scope: 'field',
            field_path: ['history'],
            is_truncated: true,
            original_size_bytes: 4096,
            preview_size_bytes: 128,
            content_type: 'application/json',
            artifact_ref: 'artifact-start-history',
            preview: '[{"role":"user","content":"旧'
          },
          tools: {
            __runtime_debug_artifact: true,
            artifact_scope: 'field',
            field_path: ['tools'],
            is_truncated: true,
            original_size_bytes: 4096,
            preview_size_bytes: 128,
            content_type: 'application/json',
            artifact_ref: 'artifact-start-tools',
            preview: '[{"name":"read'
          }
        },
        output_payload: {},
        error_payload: null,
        metrics_payload: {},
        debug_payload: {},
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      checkpoints: [],
      events: []
    });

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-start" />
      </AppProviders>
    );

    const inputJson = await screen.findByLabelText('输入 JSON');
    expect(inputJson).toHaveTextContent('总结退款政策');
    expect(inputJson).toHaveTextContent('ApiBaseUrl');
    expect(inputJson).not.toHaveTextContent('start_input_summary');
    expect(inputJson).toHaveTextContent('artifact-start-history');

    fireEvent.click(await screen.findByRole('button', { name: '加载完整值' }));

    expect(runtimeApi.fetchRuntimeDebugArtifacts).toHaveBeenCalledWith(
      'app-1',
      ['artifact-start-history', 'artifact-start-tools']
    );
    await waitFor(() =>
      expect(screen.getByLabelText('输入 JSON')).toHaveTextContent('旧问题')
    );
    expect(screen.getByLabelText('输入 JSON')).toHaveTextContent('read_file');
    expect(
      screen.queryByRole('button', { name: '加载完整值' })
    ).not.toBeInTheDocument();
  });

  test('renders data processing for non-LLM nodes when debug payload exists', async () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue({
      flow_run: {
        id: 'run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_node_preview',
        status: 'succeeded',
        target_node_id: 'node-tool',
        input_payload: {},
        output_payload: {},
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        created_at: '2026-04-17T09:00:00Z'
      },
      node_run: {
        id: 'node-run-1',
        flow_run_id: 'run-1',
        node_id: 'node-tool',
        node_type: 'tool',
        node_alias: 'Tool',
        status: 'succeeded',
        input_payload: { query: '退款' },
        output_payload: { result: 'ok' },
        error_payload: null,
        metrics_payload: {},
        debug_payload: {
          provider_events: [
            {
              type: 'tool_request',
              url: 'https://example.test/search'
            }
          ]
        },
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      checkpoints: [],
      events: []
    });

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-tool" />
      </AppProviders>
    );

    expect(await screen.findByLabelText('数据处理 JSON')).toHaveTextContent(
      'example.test'
    );
  });

  test('always renders data processing when debug payload is empty', async () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue({
      flow_run: {
        id: 'run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_node_preview',
        status: 'succeeded',
        target_node_id: 'node-code',
        input_payload: {},
        output_payload: {},
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        created_at: '2026-04-17T09:00:00Z'
      },
      node_run: {
        id: 'node-run-1',
        flow_run_id: 'run-1',
        node_id: 'node-code',
        node_type: 'code',
        node_alias: 'Code',
        status: 'succeeded',
        input_payload: { value: 1 },
        output_payload: { value: 2 },
        error_payload: null,
        metrics_payload: {},
        debug_payload: {},
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      checkpoints: [],
      events: []
    });

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-code" />
      </AppProviders>
    );

    expect(await screen.findByLabelText('数据处理 JSON')).toHaveTextContent(
      '{}'
    );
  });

  test('renders code console logs only inside data processing payload', async () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue({
      flow_run: {
        id: 'run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_node_preview',
        status: 'succeeded',
        target_node_id: 'node-code',
        input_payload: {},
        output_payload: {},
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        created_at: '2026-04-17T09:00:00Z'
      },
      node_run: {
        id: 'node-run-1',
        flow_run_id: 'run-1',
        node_id: 'node-code',
        node_type: 'code',
        node_alias: 'Code',
        status: 'succeeded',
        input_payload: { arg1: '1', arg2: '22' },
        output_payload: { result: '122' },
        error_payload: null,
        metrics_payload: {},
        debug_payload: {
          console_logs: [
            {
              level: 'log',
              message: '122',
              args: ['122']
            },
            {
              level: 'warn',
              message: 'check arg2',
              args: ['check arg2']
            }
          ]
        },
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      checkpoints: [],
      events: []
    });

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-code" />
      </AppProviders>
    );

    const processJson = await screen.findByLabelText('数据处理 JSON');
    expect(screen.queryByLabelText('控制台日志')).not.toBeInTheDocument();
    expect(processJson).toHaveTextContent('"console_logs"');
    expect(processJson).toHaveTextContent('"level": "info"');
    expect(processJson).not.toHaveTextContent('"level": "log"');
    expect(processJson).toHaveTextContent('122');
    expect(processJson).toHaveTextContent('check arg2');
  });

  test('renders warning state when runtime payload is malformed', async () => {
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue({
      node_run: null
    } as never);

    render(
      <AppProviders>
        <NodeLastRunTab applicationId="app-1" nodeId="node-llm" />
      </AppProviders>
    );

    expect(await screen.findByText('上次运行数据异常')).toBeInTheDocument();
  });
});
