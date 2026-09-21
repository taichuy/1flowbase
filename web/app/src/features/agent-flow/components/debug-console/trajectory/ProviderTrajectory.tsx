import { useState, type ReactNode } from 'react';
import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Descriptions,
  Empty,
  Modal,
  Space,
  Spin,
  Table,
  Tabs,
  Tag,
  Typography
} from 'antd';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import type { ProviderTrajectoryStep } from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { i18nText } from '../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../shared/i18n/format';
import './provider-trajectory.css';

function stepKind(step: ProviderTrajectoryStep) {
  switch (step.metadata.kind) {
    case 'request':
      return i18nText('agentFlow', 'trajectory.request');
    case 'response_head':
      return i18nText('agentFlow', 'trajectory.response_head');
    case 'response_body':
      return i18nText('agentFlow', 'trajectory.response_body');
    case 'message':
      return i18nText('agentFlow', 'trajectory.message');
    case 'stream_end':
      return i18nText('agentFlow', 'trajectory.stream_end');
  }
}

export function ProviderTrajectory({
  runId,
  nodeRunId,
  loader,
  executionContent
}: {
  runId: string;
  nodeRunId: string;
  loader: ConversationLogTraceLoader;
  executionContent?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState('protocol');
  const [selected, setSelected] = useState<ProviderTrajectoryStep | null>(null);
  const pages = useInfiniteQuery({
    queryKey: ['provider-trajectory', runId, nodeRunId],
    enabled: open && Boolean(loader.loadTrajectory),
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) =>
      loader.loadTrajectory!(runId, nodeRunId, pageParam),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    refetchOnWindowFocus: false
  });
  const body = useQuery({
    queryKey: [
      'provider-trajectory-body',
      runId,
      nodeRunId,
      selected?.event_id
    ],
    enabled:
      open &&
      tab === 'protocol' &&
      Boolean(selected && loader.loadTrajectoryBody),
    queryFn: () =>
      loader.loadTrajectoryBody!(runId, nodeRunId, selected!.event_id),
    refetchOnWindowFocus: false,
    staleTime: Infinity
  });
  const overview = pages.data?.pages[0];
  const items = pages.data?.pages.flatMap((page) => page.items) ?? [];
  const integrityLabel =
    overview?.integrity === 'complete'
      ? i18nText('agentFlow', 'trajectory.complete')
      : overview?.integrity === 'incomplete'
        ? i18nText('agentFlow', 'trajectory.incomplete')
        : i18nText('agentFlow', 'trajectory.unavailable');

  return (
    <>
      <Button
        size="small"
        type="text"
        icon={<ApartmentOutlined />}
        aria-label={i18nText('agentFlow', 'trajectory.title')}
        title={i18nText('agentFlow', 'trajectory.title')}
        onClick={(event) => {
          event.stopPropagation();
          setOpen(true);
        }}
      />
      <Modal
        open={open}
        onCancel={() => setOpen(false)}
        footer={null}
        width={1080}
        title={i18nText('agentFlow', 'trajectory.title')}
        destroyOnHidden
      >
        <Tabs
          activeKey={tab}
          onChange={setTab}
          items={[
            {
              key: 'protocol',
              label: i18nText('agentFlow', 'trajectory.protocol')
            },
            {
              key: 'execution',
              label: i18nText('agentFlow', 'trajectory.execution')
            }
          ]}
        />
        {tab === 'execution' ? (
          (executionContent ?? (
            <Empty
              description={i18nText('agentFlow', 'trajectory.no_execution')}
            />
          ))
        ) : (
          <>
            {pages.isLoading ? <Spin /> : null}
            {pages.isError ? (
              <Alert
                type="error"
                showIcon
                title={i18nText('agentFlow', 'auto.loading_failed')}
                action={
                  <Button onClick={() => void pages.refetch()}>
                    {i18nText('agentFlow', 'auto.retry')}
                  </Button>
                }
              />
            ) : null}
            <Space wrap className="provider-trajectory__overview">
              <Typography.Text strong>
                {i18nText('agentFlow', 'trajectory.overview')}
              </Typography.Text>
              <Tag
                color={
                  overview?.integrity === 'incomplete' ? 'warning' : undefined
                }
              >
                {integrityLabel}
              </Tag>
              <Typography.Text>
                {i18nText('agentFlow', 'trajectory.observations', {
                  count: overview?.observation_count ?? 0
                })}
              </Typography.Text>
              {overview?.persist_failed_count ? (
                <Typography.Text type="warning">
                  {i18nText('agentFlow', 'trajectory.failed_records', {
                    count: overview.persist_failed_count
                  })}
                </Typography.Text>
              ) : null}
            </Space>
            {items.length ? (
              <>
                <nav
                  className="provider-trajectory__sequence"
                  aria-label={i18nText('agentFlow', 'trajectory.overview')}
                >
                  {items.map((step) => (
                    <Button
                      key={step.event_id}
                      size="small"
                      type={
                        selected?.event_id === step.event_id
                          ? 'primary'
                          : 'default'
                      }
                      title={`${formatDateTime(step.created_at)} · ${stepKind(step)}`}
                      onClick={() => setSelected(step)}
                    >
                      {step.event_sequence}
                    </Button>
                  ))}
                </nav>
                <div className="provider-trajectory__workspace">
                  <section
                    aria-label={i18nText('agentFlow', 'trajectory.steps')}
                  >
                    <Table<ProviderTrajectoryStep>
                      size="small"
                      rowKey="event_id"
                      pagination={false}
                      dataSource={items}
                      scroll={{ y: 400 }}
                      onRow={(step) => ({ onClick: () => setSelected(step) })}
                      rowClassName={(step) =>
                        selected?.event_id === step.event_id
                          ? 'provider-trajectory__selected'
                          : ''
                      }
                      columns={[
                        { title: '#', dataIndex: 'event_sequence', width: 58 },
                        {
                          title: i18nText('agentFlow', 'trajectory.step'),
                          render: (_, step) => (
                            <Button
                              type="link"
                              size="small"
                              onClick={() => setSelected(step)}
                            >
                              {stepKind(step)}
                            </Button>
                          )
                        },
                        {
                          title: i18nText('agentFlow', 'trajectory.direction'),
                          render: (_, step) =>
                            step.metadata.direction === 'sent'
                              ? i18nText('agentFlow', 'trajectory.sent')
                              : i18nText('agentFlow', 'trajectory.received')
                        },
                        {
                          title: i18nText('agentFlow', 'trajectory.attempt'),
                          render: (_, step) =>
                            step.metadata.provider_attempt_index
                        }
                      ]}
                    />
                    {pages.hasNextPage ? (
                      <Button
                        loading={pages.isFetchingNextPage}
                        onClick={() => void pages.fetchNextPage()}
                      >
                        {i18nText('agentFlow', 'trajectory.more')}
                      </Button>
                    ) : null}
                  </section>
                  <section
                    className="provider-trajectory__inspector"
                    aria-label={i18nText('agentFlow', 'trajectory.inspector')}
                  >
                    {selected ? (
                      <>
                        <Descriptions
                          size="small"
                          column={1}
                          items={[
                            {
                              key: 'protocol',
                              label: i18nText(
                                'agentFlow',
                                'trajectory.protocol'
                              ),
                              children: `${selected.metadata.protocol} / ${selected.metadata.transport}`
                            },
                            {
                              key: 'invocation',
                              label: i18nText(
                                'agentFlow',
                                'trajectory.invocation'
                              ),
                              children: selected.metadata.invocation_id
                            },
                            {
                              key: 'sequence',
                              label: i18nText(
                                'agentFlow',
                                'trajectory.sequence'
                              ),
                              children: selected.metadata.sequence
                            },
                            {
                              key: 'time',
                              label: i18nText('agentFlow', 'trajectory.time'),
                              children: formatDateTime(selected.created_at)
                            },
                            ...(selected.metadata.status === undefined
                              ? []
                              : [
                                  {
                                    key: 'status',
                                    label: i18nText(
                                      'agentFlow',
                                      'trajectory.status'
                                    ),
                                    children: selected.metadata.status
                                  }
                                ])
                          ]}
                        />
                        {body.isLoading ? <Spin /> : null}
                        {body.isError ? (
                          <Alert
                            type="error"
                            showIcon
                            title={i18nText('agentFlow', 'auto.loading_failed')}
                            action={
                              <Button onClick={() => void body.refetch()}>
                                {i18nText('agentFlow', 'auto.retry')}
                              </Button>
                            }
                          />
                        ) : null}
                        {body.data ? (
                          <>
                            <Tag>{body.data.encoding}</Tag>
                            <pre className="provider-trajectory__body">
                              {body.data.body}
                            </pre>
                          </>
                        ) : null}
                      </>
                    ) : (
                      <Empty
                        description={i18nText(
                          'agentFlow',
                          'trajectory.select_step'
                        )}
                      />
                    )}
                  </section>
                </div>
              </>
            ) : !pages.isLoading && !pages.isError ? (
              <Empty
                description={i18nText('agentFlow', 'trajectory.unavailable')}
              />
            ) : null}
          </>
        )}
      </Modal>
    </>
  );
}
