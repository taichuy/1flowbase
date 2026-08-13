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

describe('ApplicationLogsPage - artifacts trace floating detail', () => {
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

  test('renders fusion route branch summaries as trace sub nodes', async () => {
    const detail = sampleRunDetail();
    const llmNodeRun = detail.node_runs[0]!;
    detail.stitched_trace = [
      {
        source_flow_run: {
          ...detail.flow_run,
          id: 'run-prior-fusion',
          status: 'succeeded',
          started_at: '2026-04-17T08:59:50Z',
          finished_at: '2026-04-17T08:59:59Z'
        },
        node_runs: [
          {
            ...llmNodeRun,
            id: 'node-run-prior-fusion-llm',
            flow_run_id: 'run-prior-fusion',
            output_payload: {
              text: 'main merged fusion review'
            },
            debug_payload: {
              llm_rounds: [
                {
                  round_index: 0,
                  assistant: {
                    role: 'assistant',
                    content: 'need fusion review',
                    tool_calls: [
                      {
                        id: 'call_fusion',
                        name: 'fusion_review'
                      }
                    ]
                  }
                },
                {
                  round_index: 1,
                  tool_results: [
                    {
                      tool_call_id: 'call_fusion',
                      name: 'fusion_review',
                      content: 'panel A says strict\npanel B says flexible'
                    }
                  ]
                },
                {
                  round_index: 2,
                  assistant: {
                    role: 'assistant',
                    content: 'main merged fusion review'
                  }
                }
              ],
              visible_internal_llm_tool_trace: [
                {
                  __runtime_debug_artifact: true,
                  kind: 'visible_internal_llm_tool_trace',
                  preview_kind: 'visible_internal_llm_tool_trace',
                  artifact_ref: 'artifact-fusion-route',
                  route_kind: 'fusion',
                  tool_call_id: 'call_fusion',
                  tool_name: 'fusion_review',
                  status: 'succeeded',
                  route_model: 'fusion-main-v1',
                  target_node_id: 'node-panel-a',
                  route_node_id: 'node-panel-a',
                  route_node_alias: 'Fusion fan-in',
                  returned_to_main: true,
                  main_resume: true,
                  branch_count: 2,
                  branch_summaries: [
                    {
                      node_id: 'node-panel-a',
                      node_alias: 'Risk Panel',
                      node_type: 'llm',
                      status: 'succeeded',
                      route_model: 'risk-v1',
                      output_summary: {
                        kind: 'text',
                        preview: 'panel A says strict',
                        char_count: 19,
                        truncated: false
                      }
                    },
                    {
                      node_id: 'node-panel-b',
                      node_alias: 'Support Panel',
                      node_type: 'llm',
                      status: 'succeeded',
                      route_model: 'support-v1',
                      output_summary: {
                        kind: 'text',
                        preview: 'panel B says flexible',
                        char_count: 21,
                        truncated: false
                      }
                    }
                  ],
                  fan_in: {
                    mode: 'bounded_parallel_panel',
                    branch_count: 2,
                    returned_to_main: true,
                    main_resume: true
                  }
                }
              ]
            },
            started_at: '2026-04-17T08:59:51Z',
            finished_at: '2026-04-17T08:59:58Z'
          }
        ],
        callback_tasks: [],
        events: []
      }
    ];
    currentRunDetail = detail;
    runtimeApi.fetchRuntimeDebugArtifact.mockImplementation(
      async (_applicationId: string, artifactRef: string) => {
        if (artifactRef === 'artifact-fusion-route') {
          return {
            kind: 'visible_internal_llm_tool_trace',
            route_kind: 'fusion',
            tool_call_id: 'call_fusion',
            tool_name: 'fusion_review',
            status: 'succeeded',
            branch_traces: [
              {
                event_type: 'visible_internal_llm_tool_completed',
                node_id: 'node-panel-a',
                node_alias: 'Risk Panel',
                node_type: 'llm',
                status: 'succeeded',
                route_model: 'risk-v1',
                input_payload: {
                  user_prompt: 'review refund policy risk',
                  model: 'risk-v1'
                },
                debug_payload: {
                  provider_debug: 'risk panel debug metadata',
                  llm_rounds: [
                    {
                      round_index: 0,
                      assistant: {
                        content: 'risk needs branch lookup',
                        tool_calls: [
                          {
                            id: 'call_branch_policy',
                            name: 'branch_policy_lookup'
                          }
                        ]
                      }
                    },
                    {
                      round_index: 1,
                      tool_results: [
                        {
                          tool_call_id: 'call_branch_policy',
                          name: 'branch_policy_lookup',
                          content: 'branch policy lookup result'
                        }
                      ]
                    },
                    {
                      round_index: 2,
                      assistant: {
                        content: 'risk result'
                      }
                    }
                  ]
                },
                output_payload: {
                  text: 'panel A says strict',
                  provider_route: {
                    model: 'risk-v1'
                  }
                },
                output_summary: {
                  kind: 'text',
                  preview: 'panel A says strict',
                  char_count: 19,
                  truncated: false
                }
              },
              {
                event_type: 'visible_internal_llm_tool_completed',
                node_id: 'node-panel-b',
                node_alias: 'Support Panel',
                node_type: 'llm',
                status: 'succeeded',
                route_model: 'support-v1',
                input_payload: {
                  user_prompt: 'review refund policy support',
                  model: 'support-v1'
                },
                debug_payload: {
                  llm_rounds: []
                },
                output_payload: {
                  text: 'panel B says flexible',
                  provider_route: {
                    model: 'support-v1'
                  }
                },
                output_summary: {
                  kind: 'text',
                  preview: 'panel B says flexible',
                  char_count: 21,
                  truncated: false
                }
              }
            ],
            fan_in: {
              mode: 'bounded_parallel_panel',
              branch_count: 2,
              returned_to_main: true,
              main_resume: true
            }
          };
        }

        throw new Error(`unexpected artifact: ${artifactRef}`);
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

    const llmTraceNode = lastElement(
      await within(logPanel).findAllByRole('button', { name: /LLM/ }),
      'expected routed LLM trace node'
    );
    fireEvent.click(llmTraceNode);
    const nodeDetail = await openLazyLlmNodeDetail(logPanel);

    const toolsNode = await within(nodeDetail).findByRole('button', {
      name: /工具 1 次工具回调/
    });
    expect(toolsNode).toHaveAttribute('aria-expanded', 'true');

    const toolCallbackNode = within(logPanel).getByRole('button', {
      name: /fusion_review/
    });
    expect(toolCallbackNode).toHaveTextContent('fusion');
    fireEvent.click(toolCallbackNode);

    const routeNode = within(logPanel).getByTestId('debug-llm-route-node');
    await waitFor(() =>
      expect(runtimeApi.fetchRuntimeDebugArtifact).toHaveBeenCalledWith(
        'app-1',
        'artifact-fusion-route'
      )
    );
    await waitFor(() =>
      expect(within(routeNode).queryByText('加载中')).not.toBeInTheDocument()
    );
    expect(routeNode).not.toHaveTextContent('Fusion fan-in');
    expect(routeNode).toHaveTextContent('执行成功');
    expect(
      within(routeNode).getAllByTestId('debug-workflow-node-item')
    ).toHaveLength(2);
    const branchNodes = within(routeNode).getAllByTestId(
      'debug-llm-route-branch-node'
    );
    expect(branchNodes).toHaveLength(2);
    expect(branchNodes[0]).toHaveTextContent('Risk Panel');
    expect(branchNodes[1]).toHaveTextContent('Support Panel');
    const firstBranchTrigger = within(branchNodes[0]).getByRole('button', {
      name: /Risk Panel/
    });
    expect(firstBranchTrigger).toHaveAttribute('aria-expanded', 'false');
    expect(branchNodes[0]).not.toHaveTextContent('risk-v1');
    fireEvent.click(firstBranchTrigger);
    expect(firstBranchTrigger).toHaveAttribute('aria-expanded', 'true');
    expect(branchNodes[0]).toHaveTextContent('risk-v1');
    const firstBranchToolsNode = within(branchNodes[0]).getByRole('button', {
      name: /工具 1 次工具回调/
    });
    expect(firstBranchToolsNode).toHaveAttribute('aria-expanded', 'true');
    expect(
      within(branchNodes[0]).getByRole('button', {
        name: /branch_policy_lookup/
      })
    ).toBeInTheDocument();
    expect(
      within(branchNodes[0]).getByLabelText('输入 JSON')
    ).toHaveTextContent('review refund policy risk');
    expect(
      within(branchNodes[0]).getByLabelText('数据处理 JSON')
    ).toHaveTextContent('risk panel debug metadata');
    expect(
      within(branchNodes[0]).getByLabelText('数据处理 JSON')
    ).not.toHaveTextContent('branch_policy_lookup');
    expect(
      within(branchNodes[0]).getByLabelText('输出 JSON')
    ).toHaveTextContent('panel A says strict');
    expect(
      within(branchNodes[0]).queryByText('visible_internal_llm_tool_completed')
    ).not.toBeInTheDocument();
    fireEvent.click(firstBranchTrigger);
    expect(firstBranchTrigger).toHaveAttribute('aria-expanded', 'false');
    expect(
      within(branchNodes[0]).queryByLabelText('输入 JSON')
    ).not.toBeInTheDocument();
    expect(
      within(routeNode).queryByLabelText('fusion JSON')
    ).not.toBeInTheDocument();
  }, 20_000);

  test('keeps expanded trace tools and loaded tool details across floating window activation', async () => {
    const detail = sampleRunDetail();
    const llmNodeRun = detail.node_runs[0]!;
    detail.node_runs = [
      {
        ...llmNodeRun,
        id: 'node-run-llm-1',
        debug_payload: {
          llm_rounds: {
            __runtime_debug_artifact: true,
            artifact_ref: 'artifact-llm-rounds',
            tool_callbacks: [
              {
                id: 'call_weather',
                name: 'lookup_weather',
                callback_status: 'returned',
                execution_status: 'succeeded',
                artifact_ref: 'artifact-tool-weather'
              }
            ]
          },
          visible_internal_llm_tool_trace: [
            {
              __runtime_debug_artifact: true,
              kind: 'visible_internal_llm_tool_trace',
              preview_kind: 'visible_internal_llm_tool_trace',
              artifact_ref: 'artifact-route-weather',
              tool_call_id: 'call_weather',
              tool_name: 'lookup_weather',
              route_model: 'mimo-v2.5',
              returned_to_main: true,
              main_resume: true,
              route_output_summary: {
                kind: 'text',
                preview: 'weather route said warm',
                char_count: 23,
                truncated: false
              }
            }
          ]
        }
      },
      {
        ...llmNodeRun,
        id: 'node-run-llm-2',
        debug_payload: {},
        started_at: '2026-04-17T09:00:01Z',
        finished_at: '2026-04-17T09:00:02Z'
      }
    ];
    currentRunDetail = detail;
    runtimeApi.fetchRuntimeDebugArtifact.mockImplementation(
      async (_applicationId: string, artifactRef: string) => {
        if (artifactRef === 'artifact-tool-weather') {
          return {
            id: 'call_weather',
            name: 'lookup_weather',
            callback_status: 'returned',
            execution_status: 'succeeded',
            request_payload: {
              city: 'Shanghai'
            },
            callback_payload: {
              temperature: 'warm'
            },
            parsed_result: {
              ok: true
            }
          };
        }
        if (artifactRef === 'artifact-route-weather') {
          return {
            kind: 'visible_internal_llm_tool_trace',
            tool_call_id: 'call_weather',
            route: {
              model: 'mimo-v2.5'
            },
            returned_to_main: true,
            main_resume: true,
            main_resume_output: {
              content: 'main saw weather route'
            }
          };
        }

        throw new Error(`unexpected artifact: ${artifactRef}`);
      }
    );
    runtimeApi.fetchRuntimeDebugArtifacts.mockImplementation(
      async (_applicationId: string, artifactRefs: string[]) => ({
        artifacts: artifactRefs.map((artifactRef) => {
          if (artifactRef === 'artifact-route-weather') {
            return {
              artifact_ref: artifactRef,
              value: {
                kind: 'visible_internal_llm_tool_trace',
                tool_call_id: 'call_weather',
                route: {
                  model: 'mimo-v2.5'
                },
                returned_to_main: true,
                main_resume: true,
                main_resume_output: {
                  content: 'main saw weather route'
                }
              }
            };
          }

          throw new Error(`unexpected artifact: ${artifactRef}`);
        })
      })
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

    const toolsNode = await within(nodeDetail).findByRole('button', {
      name: /工具 1 次工具回调/
    });
    expect(toolsNode).toHaveAttribute('aria-expanded', 'true');

    const toolCallbackNode = within(logPanel).getByRole('button', {
      name: /lookup_weather/
    });
    expect(toolCallbackNode).toHaveTextContent('智能路由');
    expect(toolCallbackNode).not.toHaveTextContent('路由模型 mimo-v2.5');
    expect(toolCallbackNode).not.toHaveTextContent('weather route said warm');
    fireEvent.click(toolCallbackNode);

    await waitFor(() =>
      expect(runtimeApi.fetchRuntimeDebugArtifact).toHaveBeenCalledTimes(1)
    );
    expect(runtimeApi.fetchRuntimeDebugArtifact).toHaveBeenCalledWith(
      'app-1',
      'artifact-tool-weather'
    );
    const routeNode = within(logPanel).getByTestId('debug-llm-route-node');
    const routeTraceJson = within(routeNode).getByLabelText('智能路由 JSON');
    expect(routeTraceJson).toHaveTextContent('weather route said warm');
    fireEvent.click(
      within(routeNode).getByRole('button', {
        name: '加载完整值'
      })
    );
    await waitFor(() =>
      expect(runtimeApi.fetchRuntimeDebugArtifacts).toHaveBeenCalledWith(
        'app-1',
        ['artifact-route-weather']
      )
    );
    await waitFor(() =>
      expect(
        within(routeNode).getByLabelText('智能路由 JSON')
      ).toHaveTextContent('main saw weather route')
    );

    fireEvent.mouseDown(
      screen.getByTestId('application-logs-floating-run-detail')
    );

    expect(
      within(logPanel).getByRole('button', {
        name: /工具 1 次工具回调/
      })
    ).toHaveAttribute('aria-expanded', 'true');
    expect(
      within(logPanel).getByRole('button', {
        name: /lookup_weather/
      })
    ).toHaveAttribute('aria-expanded', 'true');

    fireEvent.click(llmTraceNode);
    expect(
      within(logPanel).queryByRole('button', {
        name: /lookup_weather/
      })
    ).not.toBeInTheDocument();

    fireEvent.click(llmTraceNode);
    expect(
      within(logPanel).getByRole('button', {
        name: /工具 1 次工具回调/
      })
    ).toHaveAttribute('aria-expanded', 'true');
    expect(
      within(logPanel).getByRole('button', {
        name: /lookup_weather/
      })
    ).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(
      within(logPanel).getByRole('button', {
        name: /lookup_weather/
      })
    );

    await waitFor(() =>
      expect(runtimeApi.fetchRuntimeDebugArtifact).toHaveBeenCalledTimes(1)
    );
  }, 20_000);

  test('does not offer run log details for imported context messages', async () => {
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationMessagesPage([
        {
          id: 'msg-history-system',
          flow_run_id: null,
          role: 'system',
          content: '你是项目助手',
          sequence: 1,
          started_at: '2026-04-17T08:58:59Z',
          finished_at: '2026-04-17T08:59:00Z'
        },
        {
          id: 'msg-history-user',
          flow_run_id: null,
          role: 'user',
          content: '外部传入的问题',
          sequence: 2,
          started_at: '2026-04-17T08:59:00Z',
          finished_at: '2026-04-17T08:59:01Z'
        },
        {
          id: 'msg-history-assistant',
          flow_run_id: null,
          role: 'assistant',
          content: '外部传入的回答',
          sequence: 3,
          started_at: '2026-04-17T08:59:01Z',
          finished_at: '2026-04-17T08:59:02Z'
        },
        {
          id: 'msg-run-1-user',
          flow_run_id: 'run-1',
          role: 'user',
          content: '总结退款政策',
          sequence: 4,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z'
        },
        {
          id: 'msg-run-1-assistant',
          flow_run_id: 'run-1',
          role: 'assistant',
          content: '退款政策摘要',
          sequence: 5,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z'
        }
      ])
    );

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('run-1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    const conversation = await screen.findByTestId(
      'debug-conversation-messages'
    );
    expect(await within(conversation).findByText('System')).toBeInTheDocument();
    expect(within(conversation).getByText('你是项目助手')).toBeInTheDocument();
    expect(
      await within(conversation).findByText('外部传入的问题')
    ).toBeInTheDocument();
    expect(
      within(conversation).getByText('外部传入的回答')
    ).toBeInTheDocument();
    expect(
      within(conversation).getAllByRole('button', {
        name: '查看对话日志'
      })
    ).toHaveLength(1);
  }, 20_000);
});
