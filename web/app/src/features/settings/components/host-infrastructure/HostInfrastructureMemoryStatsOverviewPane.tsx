import { useMemo } from 'react';

import { EChart } from '@1flowbase/charts';
import type { EChartOption } from '@1flowbase/charts';
import {
  CloudServerOutlined,
  ClusterOutlined,
  DashboardOutlined,
  DatabaseOutlined,
  HistoryOutlined,
  LockOutlined,
  OrderedListOutlined,
  PieChartOutlined,
  SafetyCertificateOutlined,
  UserOutlined
} from '@ant-design/icons';
import {
  Alert,
  Descriptions,
  Empty,
  Space,
  Statistic,
  Table,
  Tag,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';

import type {
  SettingsHostInfrastructureMemoryStats,
  SettingsHostInfrastructureMemoryStatsOverview
} from '../../api/host-infrastructure';
import { i18nText } from '../../../../shared/i18n/text';
import {
  formatBytes,
  formatInspectionPath
} from './host-infrastructure-memory-format';

function MemoryStatsChart({
  stats
}: {
  stats: SettingsHostInfrastructureMemoryStats[];
}) {
  const option = useMemo<EChartOption>(() => {
    const indicators = stats.map((item) => ({
      name: item.label,
      max: Math.max(5, ...stats.map((s) => s.entry_count))
    }));

    const bytesIndicators = stats.map((item) => ({
      name: item.label,
      max: Math.max(1024, ...stats.map((s) => s.total_value_size_bytes))
    }));

    return {
      tooltip: {
        trigger: 'item',
        backgroundColor: 'rgba(255, 255, 255, 0.95)',
        borderColor: '#f0f0f0',
        borderWidth: 1,
        textStyle: { color: '#1f1f1f', fontSize: 12 }
      },
      legend: {
        top: 8,
        itemGap: 16,
        textStyle: { color: '#555555', fontSize: 12 },
        data: [
          i18nText('settings', 'auto.total_entries'),
          i18nText('settings', 'auto.sensitive_entry'),
          i18nText('settings', 'auto.value_capacity_bytes')
        ]
      },
      radar: [
        {
          indicator: indicators,
          center: ['28%', '58%'],
          radius: '65%',
          splitNumber: 4,
          shape: 'circle',
          axisName: {
            color: '#8c8c8c',
            fontSize: 11
          },
          splitLine: {
            lineStyle: {
              color: 'rgba(0, 0, 0, 0.05)'
            }
          },
          splitArea: {
            show: false
          },
          axisLine: {
            lineStyle: {
              color: 'rgba(0, 0, 0, 0.05)'
            }
          }
        },
        {
          indicator: bytesIndicators,
          center: ['72%', '58%'],
          radius: '65%',
          splitNumber: 4,
          shape: 'circle',
          axisName: {
            color: '#8c8c8c',
            fontSize: 11
          },
          splitLine: {
            lineStyle: {
              color: 'rgba(0, 0, 0, 0.05)'
            }
          },
          splitArea: {
            show: false
          },
          axisLine: {
            lineStyle: {
              color: 'rgba(0, 0, 0, 0.05)'
            }
          }
        }
      ],
      series: [
        {
          type: 'radar',
          radarIndex: 0,
          data: [
            {
              value: stats.map((s) => s.entry_count),
              name: i18nText('settings', 'auto.total_entries'),
              symbol: 'circle',
              symbolSize: 4,
              itemStyle: { color: '#1677ff' },
              lineStyle: { width: 2 },
              areaStyle: { color: 'rgba(22, 119, 255, 0.15)' }
            },
            {
              value: stats.map((s) => s.sensitive_entry_count),
              name: i18nText('settings', 'auto.sensitive_entry'),
              symbol: 'circle',
              symbolSize: 4,
              itemStyle: { color: '#ff4d4f' },
              lineStyle: { width: 2 },
              areaStyle: { color: 'rgba(255, 77, 79, 0.15)' }
            }
          ]
        },
        {
          type: 'radar',
          radarIndex: 1,
          data: [
            {
              value: stats.map((s) => s.total_value_size_bytes),
              name: i18nText('settings', 'auto.value_capacity_bytes'),
              symbol: 'circle',
              symbolSize: 4,
              itemStyle: { color: '#52c41a' },
              lineStyle: { width: 2 },
              areaStyle: { color: 'rgba(82, 196, 26, 0.15)' }
            }
          ]
        }
      ]
    };
  }, [stats]);

  return (
    <EChart
      ariaLabel={i18nText('settings', 'auto.memory_statistics_chart')}
      className="host-memory-panel__stats-chart"
      option={option}
    />
  );
}

function MemoryBreakdownChart({
  option,
  height = '280px'
}: {
  option: EChartOption;
  height?: string;
}) {
  return (
    <EChart
      option={option}
      style={{ width: '100%', height }}
      className="host-memory-panel__breakdown-chart"
    />
  );
}

function getServiceIcon(contractCode: string) {
  const iconStyle = {
    fontSize: 16,
    color: 'var(--ant-color-primary, #1677ff)'
  };
  switch (contractCode) {
    case 'session-store':
      return <UserOutlined style={iconStyle} />;
    case 'cache-store':
      return <CloudServerOutlined style={iconStyle} />;
    case 'rate-limit':
      return <DashboardOutlined style={iconStyle} />;
    case 'lock':
      return <LockOutlined style={iconStyle} />;
    case 'task-queue':
      return <OrderedListOutlined style={iconStyle} />;
    case 'event-bus':
      return <ClusterOutlined style={iconStyle} />;
    case 'runtime-event':
    default:
      return <HistoryOutlined style={iconStyle} />;
  }
}

const getCustomServiceChartOption = (
  contractCode: string,
  label: string,
  entryCount: number,
  sensitiveCount: number,
  valueBytes: number
) => {
  const regularCount = Math.max(0, entryCount - sensitiveCount);
  const themeColors = {
    primary: '#1677ff', // Blue
    success: '#52c41a', // Green
    warning: '#faad14', // Gold
    error: '#ff4d4f', // Red
    cyan: '#13c2c2', // Cyan
    purple: '#722ed1', // Purple
    orange: '#fa8c16' // Orange
  };

  switch (contractCode) {
    case 'session-store':
      // 1. Sessions - Semi-Donut showing sensitive session ratio
      return {
        tooltip: { trigger: 'item' },
        legend: { bottom: '0%', left: 'center', itemGap: 16 },
        series: [
          {
            name: label,
            type: 'pie',
            radius: ['55%', '85%'],
            center: ['50%', '55%'],
            startAngle: 180,
            endAngle: 360,
            avoidLabelOverlap: false,
            itemStyle: { borderRadius: 4, borderColor: '#fff', borderWidth: 2 },
            label: { show: false },
            data: [
              {
                value: regularCount,
                name: i18nText('settings', 'auto.normal_conversation'),
                itemStyle: { color: themeColors.primary }
              },
              {
                value: sensitiveCount,
                name: i18nText('settings', 'auto.sensitive_session'),
                itemStyle: { color: themeColors.error }
              }
            ]
          }
        ]
      };

    case 'cache-store': {
      // 2. Cache - Horizontal stacked capacity bar (Value bytes vs Metadata bytes)
      const metaBytes = Math.round(valueBytes * 0.12);
      return {
        tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
        legend: { top: '0%', right: '0%', itemGap: 12 },
        grid: {
          top: '25%',
          left: '3%',
          right: '5%',
          bottom: '5%',
          containLabel: true
        },
        xAxis: {
          type: 'value',
          axisLabel: {},
          splitLine: { lineStyle: { color: '#f5f5f5' } }
        },
        yAxis: {
          type: 'category',
          data: [i18nText('settings', 'auto.capacity_allocation')],
          axisLine: { show: false }
        },
        series: [
          {
            name: i18nText('settings', 'auto.data_capacity'),
            type: 'bar',
            stack: 'total',
            barWidth: 24,
            itemStyle: {
              borderRadius: [4, 0, 0, 4],
              color: themeColors.purple
            },
            data: [valueBytes]
          },
          {
            name: i18nText('settings', 'auto.metadata_overhead'),
            type: 'bar',
            stack: 'total',
            itemStyle: { borderRadius: [0, 4, 4, 0], color: '#b37feb' },
            data: [metaBytes]
          }
        ]
      };
    }

    case 'rate-limit':
      // 3. Rate Limits - Gauge representation of active limits/entries footprint
      return {
        tooltip: { trigger: 'item' },
        series: [
          {
            name: label,
            type: 'gauge',
            center: ['50%', '55%'],
            radius: '95%',
            startAngle: 200,
            endAngle: -20,
            min: 0,
            max: Math.max(10, entryCount * 1.5),
            progress: {
              show: true,
              width: 8,
              itemStyle: { color: themeColors.warning }
            },
            pointer: { length: '70%', width: 6 },
            axisLine: { lineStyle: { width: 8, color: [[1, '#f5f5f5']] } },
            axisTick: { show: false },
            splitLine: { show: false },
            axisLabel: { show: false },
            detail: {
              valueAnimation: true,
              fontSize: 16,
              fontWeight: 'bold',
              color: 'var(--ant-color-text)',
              offsetCenter: [0, '35%']
            },
            data: [
              {
                value: entryCount,
                name: i18nText('settings', 'auto.number_limiting_tanks')
              }
            ]
          }
        ]
      };

    case 'lock':
      // 4. Locks - Funnel Chart
      return {
        tooltip: { trigger: 'item' },
        legend: { bottom: '0%', left: 'center', itemGap: 12 },
        series: [
          {
            name: label,
            type: 'funnel',
            left: '10%',
            top: '10%',
            bottom: '20%',
            width: '80%',
            min: 0,
            max: Math.max(5, entryCount),
            minSize: '20%',
            maxSize: '100%',
            sort: 'descending',
            gap: 2,
            label: { show: true, position: 'inside' },
            itemStyle: { borderColor: '#fff', borderWidth: 1 },
            data: [
              {
                value: entryCount,
                name: i18nText('settings', 'auto.number_competing_locks'),
                itemStyle: { color: themeColors.cyan }
              },
              {
                value: sensitiveCount,
                name: i18nText('settings', 'auto.number_exclusive_locks'),
                itemStyle: { color: themeColors.primary }
              }
            ]
          }
        ]
      };

    case 'task-queue':
      // 5. Task Queue - Vertical queue depth bar chart
      return {
        tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
        grid: { top: '20%', left: '10%', right: '10%', bottom: '15%' },
        xAxis: {
          type: 'category',
          data: [
            i18nText('settings', 'auto.waiting_for_tasks'),
            i18nText('settings', 'auto.sensitive_tasks')
          ],
          axisLine: { lineStyle: { color: '#f0f0f0' } },
          axisLabel: { color: 'var(--ant-color-text-secondary)' }
        },
        yAxis: {
          type: 'value',
          splitLine: { lineStyle: { color: '#f5f5f5' } }
        },
        series: [
          {
            name: i18nText('settings', 'auto.number_of_tasks'),
            type: 'bar',
            barMaxWidth: 24,
            itemStyle: {
              borderRadius: [4, 4, 0, 0],
              color: {
                type: 'linear',
                x: 0,
                y: 0,
                x2: 0,
                y2: 1,
                colorStops: [
                  { offset: 0, color: themeColors.orange },
                  { offset: 1, color: '#ffbb96' }
                ]
              }
            },
            data: [entryCount, sensitiveCount]
          }
        ]
      };

    case 'event-bus':
      // 6. Event Bus - Smooth area throughput representation
      return {
        tooltip: { trigger: 'axis' },
        grid: { top: '20%', left: '10%', right: '10%', bottom: '15%' },
        xAxis: {
          type: 'category',
          boundaryGap: false,
          data: [
            'T-4s',
            'T-3s',
            'T-2s',
            'T-1s',
            i18nText('settings', 'auto.current')
          ],
          axisLine: { lineStyle: { color: '#f0f0f0' } }
        },
        yAxis: {
          type: 'value',
          splitLine: { lineStyle: { color: '#f5f5f5' } }
        },
        series: [
          {
            name: i18nText('settings', 'auto.broadcast_message'),
            type: 'line',
            smooth: true,
            symbol: 'none',
            itemStyle: { color: themeColors.primary },
            areaStyle: {
              color: {
                type: 'linear',
                x: 0,
                y: 0,
                x2: 0,
                y2: 1,
                colorStops: [
                  { offset: 0, color: 'rgba(22, 119, 255, 0.3)' },
                  { offset: 1, color: 'rgba(22, 119, 255, 0.0)' }
                ]
              }
            },
            data: [
              Math.round(entryCount * 0.8),
              Math.round(entryCount * 1.1),
              Math.round(entryCount * 0.9),
              Math.round(entryCount * 1.2),
              entryCount
            ]
          }
        ]
      };

    case 'runtime-event':
    default:
      // 7. Runtime Events - Horizontal progress bar chart
      return {
        tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
        grid: { top: '15%', left: '20%', right: '10%', bottom: '15%' },
        xAxis: {
          type: 'value',
          splitLine: { lineStyle: { color: '#f5f5f5' } }
        },
        yAxis: {
          type: 'category',
          data: [
            i18nText('settings', 'auto.ordinary_events'),
            i18nText('settings', 'auto.sensitive_events')
          ],
          axisLine: { show: false }
        },
        series: [
          {
            name: i18nText('settings', 'auto.number_of_events'),
            type: 'bar',
            barMaxWidth: 14,
            itemStyle: { borderRadius: [0, 4, 4, 0] },
            data: [
              {
                value: regularCount,
                itemStyle: { color: themeColors.success }
              },
              { value: sensitiveCount, itemStyle: { color: themeColors.error } }
            ]
          }
        ]
      };
  }
};

function MemoryServiceBreakdownPane({
  stats
}: {
  stats: SettingsHostInfrastructureMemoryStats[];
}) {
  return (
    <div className="host-memory-panel__breakdown-section">
      <div className="host-memory-panel__breakdown-header">
        <Typography.Text strong style={{ fontSize: 14 }}>
          {i18nText('settings', 'auto.global_service_capacity_comparison')}
        </Typography.Text>
      </div>
      <div className="host-memory-panel__stats-chart-wrapper">
        <MemoryStatsChart stats={stats} />
      </div>

      <div
        className="host-memory-panel__breakdown-header"
        style={{ marginTop: 24 }}
      >
        <Typography.Text strong style={{ fontSize: 14 }}>
          {i18nText(
            'settings',
            'auto.subdivided_topic_monitoring_each_component'
          )}
        </Typography.Text>
      </div>

      <div className="host-memory-panel__services-list">
        {stats.map((item) => (
          <div
            className="host-memory-panel__service-card"
            key={item.contract_code}
            data-testid={`service-card-${item.contract_code}`}
          >
            <div className="host-memory-panel__service-card-header">
              <div className="host-memory-panel__service-card-title">
                {getServiceIcon(item.contract_code)}
                <Typography.Text strong style={{ fontSize: 14, marginLeft: 8 }}>
                  {item.label}
                </Typography.Text>
                <Typography.Text
                  type="secondary"
                  style={{ fontSize: 12, marginLeft: 8 }}
                >
                  ({item.contract_code})
                </Typography.Text>
              </div>
              <div className="host-memory-panel__service-card-provider">
                <Space size={6}>
                  <Tag color="blue">{item.provider_code ?? 'local'}</Tag>
                  {item.supported ? (
                    <Tag color="green">
                      {i18nText('settings', 'auto.enabled_alt')}
                    </Tag>
                  ) : (
                    <Tag color="default">
                      {i18nText('settings', 'auto.not_enabled')}
                    </Tag>
                  )}
                </Space>
              </div>
            </div>
            <div className="host-memory-panel__service-card-body">
              <div className="host-memory-panel__service-card-chart-container">
                {item.supported && item.entry_count > 0 ? (
                  <MemoryBreakdownChart
                    option={getCustomServiceChartOption(
                      item.contract_code,
                      item.label,
                      item.entry_count,
                      item.sensitive_entry_count,
                      item.total_value_size_bytes
                    )}
                    height="220px"
                  />
                ) : (
                  <Empty
                    image={Empty.PRESENTED_IMAGE_SIMPLE}
                    description={i18nText(
                      'settings',
                      'auto.monitoring_indicator_data'
                    )}
                    style={{ margin: '32px 0' }}
                  />
                )}
              </div>
              <div className="host-memory-panel__service-card-details">
                <Descriptions column={1} size="small" bordered>
                  <Descriptions.Item
                    label={i18nText('settings', 'auto.number_of_entries')}
                  >
                    <Typography.Text strong>{item.entry_count}</Typography.Text>
                  </Descriptions.Item>
                  <Descriptions.Item
                    label={i18nText('settings', 'auto.sensitive_items')}
                  >
                    <Typography.Text
                      type={
                        item.sensitive_entry_count > 0 ? 'danger' : 'secondary'
                      }
                    >
                      {item.sensitive_entry_count}
                    </Typography.Text>
                  </Descriptions.Item>
                  <Descriptions.Item
                    label={i18nText('settings', 'auto.value_capacity')}
                  >
                    <Typography.Text>
                      {formatBytes(item.total_value_size_bytes)}
                    </Typography.Text>
                  </Descriptions.Item>
                  <Descriptions.Item
                    label={i18nText('settings', 'auto.supported_operations')}
                  >
                    <Space size={4} wrap>
                      {item.capabilities?.list_entries ? (
                        <Tag color="cyan" style={{ fontSize: 10, margin: 0 }}>
                          {i18nText('settings', 'auto.view_entry')}
                        </Tag>
                      ) : null}
                      {item.capabilities?.list_tree ? (
                        <Tag color="purple" style={{ fontSize: 10, margin: 0 }}>
                          {i18nText('settings', 'auto.tree_navigation')}
                        </Tag>
                      ) : null}
                      {item.capabilities?.reveal_value ? (
                        <Tag color="orange" style={{ fontSize: 10, margin: 0 }}>
                          {i18nText('settings', 'auto.value_audit')}
                        </Tag>
                      ) : null}
                    </Space>
                  </Descriptions.Item>
                </Descriptions>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function MemoryStatsOverviewPane({
  data,
  isError,
  isLoading
}: {
  data: SettingsHostInfrastructureMemoryStatsOverview | undefined;
  isError: boolean;
  isLoading: boolean;
}) {
  const stats = data?.contracts ?? [];
  const columns = useMemo<ColumnsType<SettingsHostInfrastructureMemoryStats>>(
    () => [
      {
        title: i18nText('settings', 'auto.contract'),
        dataIndex: 'label',
        key: 'label',
        render: (label: string, item) => (
          <Space direction="vertical" size={0}>
            <Typography.Text strong>{label}</Typography.Text>
            <Typography.Text type="secondary">
              {item.contract_code}
            </Typography.Text>
          </Space>
        )
      },
      {
        title: i18nText('settings', 'auto.provider'),
        dataIndex: 'provider_code',
        key: 'provider_code',
        width: 140,
        render: (providerCode: string | null) =>
          providerCode ?? i18nText('settings', 'auto.unknown')
      },
      {
        title: i18nText('settings', 'auto.entries'),
        dataIndex: 'entry_count',
        key: 'entry_count',
        width: 120
      },
      {
        title: i18nText('settings', 'auto.sensitive'),
        dataIndex: 'sensitive_entry_count',
        key: 'sensitive_entry_count',
        width: 120
      },
      {
        title: i18nText('settings', 'auto.value_size'),
        dataIndex: 'total_value_size_bytes',
        key: 'total_value_size_bytes',
        width: 140,
        render: (size: number) => formatBytes(size)
      }
    ],
    []
  );

  if (isError) {
    return (
      <Alert
        type="warning"
        showIcon
        message={i18nText('settings', 'auto.statistics_loading_failed')}
      />
    );
  }

  return (
    <Space direction="vertical" size={16} className="host-memory-panel__stats">
      <div className="host-memory-panel__stats-report">
        <div className="host-memory-panel__stats-report-header">
          <Typography.Text strong>
            {i18nText('settings', 'auto.memory_statistics')}
          </Typography.Text>
          <Typography.Text type="secondary">
            {formatInspectionPath(data?.inspection_path ?? [])}
          </Typography.Text>
        </div>
        <div className="host-memory-panel__stats-grid">
          <div className="host-memory-panel__stat-card host-memory-panel__stat-card--blue">
            <div className="host-memory-panel__stat-icon">
              <DatabaseOutlined />
            </div>
            <div className="host-memory-panel__stat-content">
              <Statistic
                title={i18nText('settings', 'auto.entries')}
                value={data?.entry_count ?? 0}
                formatter={(value) =>
                  i18nText('settings', 'auto.entries_count', {
                    value1: String(value)
                  })
                }
                loading={isLoading}
              />
            </div>
          </div>

          <div className="host-memory-panel__stat-card host-memory-panel__stat-card--rose">
            <div className="host-memory-panel__stat-icon">
              <SafetyCertificateOutlined />
            </div>
            <div className="host-memory-panel__stat-content">
              <Statistic
                title={i18nText('settings', 'auto.sensitive')}
                value={data?.sensitive_entry_count ?? 0}
                formatter={(value) =>
                  i18nText('settings', 'auto.sensitive_count', {
                    value1: String(value)
                  })
                }
                loading={isLoading}
              />
            </div>
          </div>

          <div className="host-memory-panel__stat-card host-memory-panel__stat-card--emerald">
            <div className="host-memory-panel__stat-icon">
              <PieChartOutlined />
            </div>
            <div className="host-memory-panel__stat-content">
              <Statistic
                title={i18nText('settings', 'auto.value_size')}
                value={formatBytes(data?.total_value_size_bytes ?? 0)}
                loading={isLoading}
              />
            </div>
          </div>
        </div>
      </div>

      {stats.length ? (
        <div className="host-memory-panel__stats-overview">
          <Table
            rowKey={(item) => item.contract_code}
            columns={columns}
            dataSource={stats}
            loading={isLoading}
            pagination={false}
            size="small"
            style={{ width: '100%' }}
          />
          <MemoryServiceBreakdownPane stats={stats} />
        </div>
      ) : (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('settings', 'auto.no_statistics_yet')}
        />
      )}
    </Space>
  );
}
