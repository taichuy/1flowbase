import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { vi } from 'vitest';

const runtimeApi = vi.hoisted(() => ({
  applicationRunsQueryKey: (
    applicationId: string,
    input?: {
      page?: number;
      pageSize?: number;
      timeRangeDays?: number | null;
      sortBy?: 'started_at' | 'finished_at' | 'created_at';
      sortOrder?: 'asc' | 'desc';
      cacheMode?: 'default' | 'refresh';
    }
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      input?.page ?? 1,
      input?.pageSize ?? 20,
      input?.timeRangeDays ?? 'all',
      input?.sortBy ?? 'started_at',
      input?.sortOrder ?? 'desc'
    ] as const,
  applicationRunTraceTreeQueryKey: (applicationId: string, runId: string) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'trace-tree'
    ] as const,
  applicationRunOverviewQueryKey: (applicationId: string, runId: string) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'overview'
    ] as const,
  applicationRunTraceNodeChildrenQueryKey: (
    applicationId: string,
    runId: string,
    traceNodeId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'trace-tree',
      traceNodeId,
      'children'
    ] as const,
  applicationRunTraceNodeContentQueryKey: (
    applicationId: string,
    runId: string,
    traceNodeId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'trace-tree',
      traceNodeId,
      'content'
    ] as const,
  applicationRunResumeTimelineQueryKey: (
    applicationId: string,
    runId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'resume-timeline'
    ] as const,
  applicationConversationMessagesQueryKey: (
    applicationId: string,
    runId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'conversation',
      'around',
      runId
    ] as const,
  applicationRunConversationMessagesQueryKey: (
    applicationId: string,
    runId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'conversation-messages'
    ] as const,
  applicationLogConversationMessagesQueryKey: (
    applicationId: string,
    externalConversationId: string,
    input?: {
      aroundRunId?: string | null;
      before?: string | null;
      after?: string | null;
      limit?: number;
    }
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'logs',
      'conversations',
      externalConversationId,
      input?.aroundRunId ?? '',
      input?.before ?? '',
      input?.after ?? '',
      input?.limit ?? 5
    ] as const,
  fetchApplicationRuns: vi.fn(),
  fetchApplicationRunOverview: vi.fn(),
  fetchApplicationRunTraceTree: vi.fn(),
  fetchApplicationRunTraceNodeChildren: vi.fn(),
  fetchApplicationRunTraceNodeContent: vi.fn(),
  fetchApplicationRunResumeTimeline: vi.fn(),
  fetchApplicationConversationMessages: vi.fn(),
  fetchApplicationLogConversationMessages: vi.fn(),
  fetchApplicationRunConversationMessages: vi.fn(),
  fetchRuntimeDebugArtifact: vi.fn(),
  fetchRuntimeDebugArtifacts: vi.fn(),
  exportApplicationRunTraceDump: vi.fn(),
  exportSelectedApplicationRunsTraceDumpZip: vi.fn(),
  resumeFlowRun: vi.fn(),
  completeCallbackTask: vi.fn()
}));

vi.mock('../../api/runtime', () => runtimeApi);

import type { ConsoleApplicationRunDetail as ApplicationRunDetail } from '@1flowbase/api-client';
import { AppProviders } from '../../../../app/AppProviders';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import { resetAuthStore } from '../../../../state/auth-store';
import { ApplicationLogsPage } from '../../pages/ApplicationLogsPage';
import {
  applicationRunsPage,
  conversationMessagesPage,
  lastElement,
  openLazyLlmNodeDetail,
  runOverviewFromDetail,
  sampleRunDetail,
  traceNodeContentFromDetail,
  traceTreeFromDetail
} from './artifacts-trace.support';

describe('ApplicationLogsPage - artifacts trace lazy tools', () => {
  let currentRunDetail: ApplicationRunDetail;
  let getBoundingClientRectSpy: { mockRestore: () => void } | undefined;
  let innerHeightSpy: { mockRestore: () => void } | undefined;
  let innerWidthSpy: { mockRestore: () => void } | undefined;
  let dateNowSpy: { mockRestore: () => void } | undefined;

  beforeEach(async () => {
    window.localStorage.clear();
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
    dateNowSpy = vi
      .spyOn(Date, 'now')
      .mockReturnValue(new Date('2026-04-18T00:00:00Z').getTime());
    runtimeApi.fetchApplicationRuns.mockReset();
    runtimeApi.fetchApplicationRunOverview.mockReset();
    runtimeApi.fetchApplicationRunTraceTree.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeChildren.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeContent.mockReset();
    runtimeApi.fetchApplicationRunResumeTimeline.mockReset();
    runtimeApi.fetchApplicationConversationMessages.mockReset();
    runtimeApi.fetchApplicationLogConversationMessages.mockReset();
    runtimeApi.fetchApplicationRunConversationMessages.mockReset();
    runtimeApi.fetchRuntimeDebugArtifact.mockReset();
    runtimeApi.fetchRuntimeDebugArtifacts.mockReset();
    currentRunDetail = sampleRunDetail();

    runtimeApi.fetchApplicationRuns.mockResolvedValue(
      applicationRunsPage([
        {
          id: 'run-1',
          run_mode: 'published_api_run' as const,
          status: 'succeeded',
          target_node_id: 'node-llm',
          title: '公开 API 退款总结',
          expand_id: 'customer-42',
          authorized_account: 'root',
          compatibility_mode: 'openai-responses-v1',
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          created_at: '2026-04-17T09:00:00Z',
          updated_at: '2026-04-17T09:00:01Z'
        }
      ])
    );
    runtimeApi.fetchApplicationRunTraceTree.mockImplementation(async () =>
      traceTreeFromDetail(currentRunDetail)
    );
    runtimeApi.fetchApplicationRunOverview.mockImplementation(async () =>
      runOverviewFromDetail(currentRunDetail)
    );
    runtimeApi.fetchApplicationRunTraceNodeChildren.mockResolvedValue({
      items: [],
      page_info: {
        has_more: false,
        next_cursor: null,
        page_size: 20
      }
    });
    runtimeApi.fetchApplicationRunTraceNodeContent.mockImplementation(
      async (_applicationId: string, _runId: string, traceNodeId: string) =>
        traceNodeContentFromDetail(currentRunDetail, traceNodeId)
    );
    runtimeApi.fetchApplicationRunResumeTimeline.mockResolvedValue({
      flow_run_status: sampleRunDetail().flow_run.status,
      callback_tasks: sampleRunDetail().callback_tasks,
      events: sampleRunDetail().events
    });
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationMessagesPage([
        {
          id: 'msg-history-system',
          flow_run_id: null,
          role: 'system',
          content: '你是项目助手',
          sequence: 1,
          started_at: '2026-04-17T08:59:00Z',
          finished_at: '2026-04-17T08:59:01Z'
        },
        {
          id: 'msg-run-1-user',
          flow_run_id: 'run-1',
          role: 'user',
          content: '总结退款政策',
          sequence: 2,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z'
        },
        {
          id: 'msg-run-1-assistant',
          flow_run_id: 'run-1',
          role: 'assistant',
          content: '退款政策摘要',
          sequence: 3,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z'
        }
      ])
    );
  });

  afterEach(() => {
    resetAuthStore();
    getBoundingClientRectSpy?.mockRestore();
    getBoundingClientRectSpy = undefined;
    innerHeightSpy?.mockRestore();
    innerHeightSpy = undefined;
    innerWidthSpy?.mockRestore();
    innerWidthSpy = undefined;
    dateNowSpy?.mockRestore();
    dateNowSpy = undefined;
  });

  test('renders lazy trace tools from children when node content omits tool index', async () => {
    const detail = sampleRunDetail();
    const llmNodeRun = detail.node_runs[0]!;
    currentRunDetail = {
      ...detail,
      node_runs: [
        {
          ...llmNodeRun,
          debug_payload: {
            llm_rounds: [
              {
                round_index: 0,
                assistant: {
                  role: 'assistant',
                  tool_calls: [
                    {
                      id: 'call-refund-policy',
                      name: 'refund_policy_lookup'
                    }
                  ]
                }
              }
            ],
            tool_callbacks: [
              {
                id: 'call-refund-policy',
                name: 'refund_policy_lookup'
              }
            ],
            debug_summary: {
              kept: true
            }
          }
        }
      ]
    };
    const llmTraceNodeId = 'trace-node-llm-lazy-tools';
    const toolsTraceNodeId = 'trace-node-tools-lazy-tools';
    const toolCallbackTraceNodeId = 'trace-node-tool-refund-policy-lazy-tools';
    runtimeApi.fetchApplicationRunTraceTree.mockResolvedValue({
      nodes: [
        {
          trace_node_id: llmTraceNodeId,
          stable_locator: 'run:run-1/node:node-run-1',
          node_kind: 'node_run',
          node_run_id: 'node-run-1',
          node_id: 'node-llm',
          node_type: 'llm',
          node_alias: 'LLM',
          status: 'succeeded',
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          duration_ms: 1000,
          metrics_payload: {},
          has_children: true,
          child_count: 1,
          has_content: true
        }
      ]
    });
    runtimeApi.fetchApplicationRunTraceNodeContent.mockResolvedValue({
      trace_node_id: llmTraceNodeId,
      node_kind: 'node_run',
      content_kind: 'node_run',
      source_refs: [],
      detail_refs: [],
      payload: {
        input_payload: llmNodeRun.input_payload,
        output_payload: llmNodeRun.output_payload,
        error_payload: llmNodeRun.error_payload,
        metrics_payload: llmNodeRun.metrics_payload,
        debug_payload: {
          debug_summary: {
            kept: true
          }
        }
      }
    });
    runtimeApi.fetchApplicationRunTraceNodeChildren.mockImplementation(
      async (_applicationId: string, _runId: string, traceNodeId: string) => {
        if (traceNodeId === llmTraceNodeId) {
          return {
            items: [
              {
                trace_node_id: toolsTraceNodeId,
                stable_locator: 'run:run-1/node:node-run-1/tools',
                node_kind: 'tool_group',
                node_run_id: null,
                node_id: null,
                node_type: 'tools',
                node_alias: 'Tools',
                status: 'completed',
                started_at: '2026-04-17T09:00:00Z',
                finished_at: '2026-04-17T09:00:01Z',
                duration_ms: null,
                metrics_payload: {},
                has_children: true,
                child_count: 1,
                has_content: false
              }
            ],
            page_info: {
              has_more: false,
              next_cursor: null,
              page_size: 20
            }
          };
        }

        if (traceNodeId === toolsTraceNodeId) {
          return {
            items: [
              {
                trace_node_id: toolCallbackTraceNodeId,
                stable_locator:
                  'run:run-1/node:node-run-1/tools/tool:call-refund-policy',
                node_kind: 'tool_callback',
                node_run_id: null,
                node_id: null,
                node_type: 'tool',
                node_alias: 'refund_policy_lookup',
                status: 'completed',
                started_at: '2026-04-17T09:00:00Z',
                finished_at: '2026-04-17T09:00:01Z',
                duration_ms: 1000,
                metrics_payload: {},
                has_children: false,
                child_count: 0,
                has_content: true
              }
            ],
            page_info: {
              has_more: false,
              next_cursor: null,
              page_size: 20
            }
          };
        }

        return {
          items: [],
          page_info: {
            has_more: false,
            next_cursor: null,
            page_size: 20
          }
        };
      }
    );

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('run-1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    const openLogButton = lastElement(
      await screen.findAllByRole(
        'button',
        { name: '查看对话日志' },
        { timeout: 8_000 }
      ),
      'expected conversation log button'
    );
    fireEvent.click(openLogButton);

    const logPanel = await screen.findByRole('complementary', {
      name: '对话日志'
    });
    fireEvent.click(within(logPanel).getByRole('tab', { name: '追踪' }));

    const llmTraceNode = await within(logPanel).findByRole('button', {
      name: /LLM/
    });
    fireEvent.click(llmTraceNode);
    const nodeDetail = await openLazyLlmNodeDetail(logPanel);

    await waitFor(() =>
      expect(runtimeApi.fetchApplicationRunTraceNodeContent).toHaveBeenCalled()
    );
    expect(
      within(nodeDetail).queryByRole('button', {
        name: /工具 .*工具回调/
      })
    ).not.toBeInTheDocument();

    const toolsGroupNode = await within(nodeDetail).findByRole('button', {
      name: /Tools/
    });
    expect(toolsGroupNode).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(toolsGroupNode);

    expect(
      await within(nodeDetail).findByRole('button', {
        name: /refund_policy_lookup/
      })
    ).toBeInTheDocument();
    expect(
      runtimeApi.fetchApplicationRunTraceNodeChildren
    ).toHaveBeenCalledWith('app-1', 'run-1', llmTraceNodeId, undefined);
    expect(
      runtimeApi.fetchApplicationRunTraceNodeChildren
    ).toHaveBeenCalledWith('app-1', 'run-1', toolsTraceNodeId, undefined);
  }, 20_000);

  test('groups repeated LLM tool callbacks under Tools from application logs', async () => {
    const detail = sampleRunDetail();
    const llmNodeRun = detail.node_runs[0]!;
    detail.flow_run.status = 'waiting_callback';
    detail.node_runs = [
      {
        ...llmNodeRun,
        id: 'node-run-llm-1',
        status: 'succeeded',
        output_payload: {
          usage: {
            total_tokens: 8035
          }
        },
        debug_payload: {
          llm_rounds: [
            {
              round_index: 0,
              assistant: {
                role: 'assistant',
                content: 'need weather',
                tool_calls: [
                  {
                    id: 'call_weather',
                    name: 'lookup_weather'
                  }
                ]
              }
            }
          ]
        },
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:03Z'
      },
      {
        ...llmNodeRun,
        id: 'node-run-llm-2',
        status: 'waiting_callback',
        output_payload: {
          tool_calls: [
            {
              id: 'call_policy'
            }
          ]
        },
        debug_payload: {
          llm_rounds: [
            {
              round_index: 1,
              assistant: {
                role: 'assistant',
                content: 'need policy',
                tool_calls: [
                  {
                    id: 'call_policy',
                    name: 'read_policy'
                  }
                ]
              }
            }
          ]
        },
        started_at: '2026-04-17T09:00:04Z',
        finished_at: null
      }
    ];
    currentRunDetail = detail;

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('run-1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    const openLogButton = lastElement(
      await screen.findAllByRole(
        'button',
        { name: '查看对话日志' },
        { timeout: 8_000 }
      ),
      'expected conversation log button'
    );
    fireEvent.click(openLogButton);

    const logPanel = await screen.findByRole('complementary', {
      name: '对话日志'
    });
    fireEvent.click(within(logPanel).getByRole('tab', { name: '追踪' }));

    await waitFor(() => {
      expect(
        within(logPanel).getAllByTestId('debug-workflow-node-row')
      ).toHaveLength(1);
    });

    const llmTraceNode = lastElement(
      await within(logPanel).findAllByRole('button', { name: /LLM/ }),
      'expected routed LLM trace node'
    );
    fireEvent.click(llmTraceNode);
    const nodeDetail = await openLazyLlmNodeDetail(logPanel);

    const toolsNode = await within(nodeDetail).findByRole('button', {
      name: /工具 2 次工具回调/
    });
    expect(toolsNode).toHaveAttribute('aria-expanded', 'true');
    expect(
      within(logPanel).queryByLabelText('工具回调索引 JSON')
    ).not.toBeInTheDocument();
    expect(
      within(logPanel).getByRole('button', {
        name: /lookup_weather/
      })
    ).toBeInTheDocument();
    expect(
      within(logPanel).getByRole('button', {
        name: /read_policy/
      })
    ).toBeInTheDocument();
    expect(
      within(logPanel).queryByText('call_weather')
    ).not.toBeInTheDocument();
    expect(within(logPanel).queryByText('call_policy')).not.toBeInTheDocument();
  }, 20_000);

  test('shows route tool callbacks from stitched conversation trace', async () => {
    const detail = sampleRunDetail();
    const llmNodeRun = detail.node_runs[0]!;
    detail.callback_tasks = [];
    detail.stitched_trace = [
      {
        source_flow_run: {
          ...detail.flow_run,
          id: 'run-prior-route',
          status: 'cancelled',
          started_at: '2026-04-17T08:59:50Z',
          finished_at: '2026-04-17T08:59:59Z'
        },
        node_runs: [
          {
            ...llmNodeRun,
            id: 'node-run-prior-llm',
            flow_run_id: 'run-prior-route',
            output_payload: {
              usage: {
                total_tokens: 33520
              }
            },
            debug_payload: {
              llm_rounds: [
                {
                  round_index: 0,
                  assistant: {
                    role: 'assistant',
                    content: 'need image route',
                    tool_calls: [
                      {
                        id: 'call_image',
                        name: 'image_llm'
                      }
                    ]
                  }
                },
                {
                  round_index: 1,
                  tool_results: [
                    {
                      tool_call_id: 'call_image',
                      name: 'image_llm',
                      content: '{"answer":"route ok"}'
                    }
                  ]
                },
                {
                  round_index: 2,
                  assistant: {
                    role: 'assistant',
                    content: 'main resumed'
                  }
                }
              ],
              visible_internal_llm_tool_trace: [
                {
                  kind: 'visible_internal_llm_tool_trace',
                  preview_kind: 'visible_internal_llm_tool_trace',
                  tool_call_id: 'call_image',
                  tool_name: 'image_llm',
                  status: 'returned_to_main',
                  route_model: 'image-route-v1',
                  target_node_id: 'node-llm-image',
                  route_node_id: 'node-llm-image',
                  route_node_alias: 'Image LLM',
                  returned_to_main: true,
                  main_resume: true,
                  route_output_summary: {
                    kind: 'text',
                    preview: 'image route completed',
                    char_count: 21,
                    truncated: false
                  },
                  final_output_summary: {
                    kind: 'text',
                    preview: 'main resumed',
                    char_count: 12,
                    truncated: false
                  }
                }
              ]
            },
            started_at: '2026-04-17T08:59:51Z',
            finished_at: '2026-04-17T08:59:58Z'
          }
        ],
        callback_tasks: [
          {
            id: 'callback-prior-image',
            flow_run_id: 'run-prior-route',
            node_run_id: 'node-run-prior-llm',
            callback_kind: 'llm_tool_calls',
            status: 'completed',
            request_payload: {
              tool_calls: [
                {
                  id: 'call_image',
                  name: 'image_llm'
                }
              ]
            },
            response_payload: null,
            external_ref_payload: null,
            created_at: '2026-04-17T08:59:52Z',
            completed_at: '2026-04-17T08:59:58Z'
          }
        ],
        events: []
      }
    ];
    currentRunDetail = detail;

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('run-1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    const openLogButton = lastElement(
      await screen.findAllByRole(
        'button',
        { name: '查看对话日志' },
        { timeout: 8_000 }
      ),
      'expected conversation log button'
    );
    fireEvent.click(openLogButton);

    const logPanel = await screen.findByRole('complementary', {
      name: '对话日志'
    });
    fireEvent.click(within(logPanel).getByRole('tab', { name: '追踪' }));

    const llmTraceNode = lastElement(
      await within(logPanel).findAllByRole('button', { name: /LLM/ }),
      'expected fusion LLM trace node'
    );
    fireEvent.click(llmTraceNode);
    const nodeDetail = await openLazyLlmNodeDetail(logPanel);

    const toolsNode = await within(nodeDetail).findByRole('button', {
      name: /工具 1 次工具回调/
    });
    expect(toolsNode).toHaveAttribute('aria-expanded', 'true');

    const toolCallbackNode = within(logPanel).getByRole('button', {
      name: /image_llm/
    });
    expect(toolCallbackNode).toHaveTextContent('智能路由');
    fireEvent.click(toolCallbackNode);

    const routeNode = within(logPanel).getByTestId('debug-llm-route-node');
    expect(routeNode).not.toHaveTextContent('进行中');
    expect(
      within(routeNode).getByLabelText('智能路由 JSON')
    ).toHaveTextContent('image-route-v1');
  }, 20_000);
});
