import { render, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { AppProviders } from '../../../app/AppProviders';
import type { ApplicationDetail } from '../api/applications';
import { ApplicationSectionState } from '../components/ApplicationSectionState';

const application: ApplicationDetail = {
  id: 'app-1',
  application_type: 'agent_flow',
  workflow_trigger_type: null,
  name: 'Support Agent',
  description: 'customer support',
  icon: 'RobotOutlined',
  icon_type: 'iconfont',
  icon_background: '#E6F7F2',
  created_by: 'user-1',
  updated_at: '2026-04-15T09:00:00Z',
  tags: [],
  sections: {
    orchestration: {
      status: 'planned',
      subject_kind: 'agent_flow',
      subject_status: 'unconfigured',
      current_subject_id: 'flow-1',
      current_draft_id: null
    },
    api: {
      status: 'enabled',
      credential_kind: 'application_api_key',
      invoke_routing_mode: 'api_key_bound_application',
      invoke_path_template: '/api/apps/app-1/invoke',
      api_capability_status: 'enabled',
      credentials_status: 'active'
    },
    logs: {
      status: 'planned',
      runs_capability_status: 'planned',
      run_object_kind: 'application_run',
      log_retention_status: 'planned'
    },
    monitoring: {
      status: 'planned',
      metrics_capability_status: 'planned',
      metrics_object_kind: 'application_metrics',
      tracing_config_status: 'not_configured'
    }
  }
};

function renderSection(
  sectionKey: Parameters<typeof ApplicationSectionState>[0]['sectionKey']
) {
  return render(
    <AppProviders>
      <ApplicationSectionState
        application={application}
        sectionKey={sectionKey}
      />
    </AppProviders>
  );
}

describe('ApplicationSectionState', () => {
  test('renders orchestration subject and draft fallback state', () => {
    renderSection('orchestration');

    expect(screen.getByRole('heading', { name: '编排' })).toBeInTheDocument();
    expect(screen.getByText('主体种类')).toBeInTheDocument();
    expect(screen.getByText('agent_flow')).toBeInTheDocument();
    expect(screen.getByText('当前主体 ID')).toBeInTheDocument();
    expect(screen.getByText('flow-1')).toBeInTheDocument();
    expect(screen.getByText('当前草稿 ID')).toBeInTheDocument();
    expect(screen.getByText('未生成')).toBeInTheDocument();
  });

  test('renders api routing contract state', () => {
    renderSection('api');

    expect(screen.getByRole('heading', { name: 'API' })).toBeInTheDocument();
    expect(screen.getByText('凭证类型')).toBeInTheDocument();
    expect(screen.getByText('application_api_key')).toBeInTheDocument();
    expect(screen.getByText('路由模式')).toBeInTheDocument();
    expect(screen.getByText('api_key_bound_application')).toBeInTheDocument();
    expect(screen.getByText('/api/apps/app-1/invoke')).toBeInTheDocument();
    expect(screen.getByText('active')).toBeInTheDocument();
  });

  test('renders monitoring observability state', () => {
    renderSection('monitoring');

    expect(screen.getByRole('heading', { name: '监控' })).toBeInTheDocument();
    expect(screen.getByText('指标对象')).toBeInTheDocument();
    expect(screen.getByText('application_metrics')).toBeInTheDocument();
    expect(screen.getByText('指标聚合状态')).toBeInTheDocument();
    expect(screen.getByText('Tracing 配置状态')).toBeInTheDocument();
    expect(screen.getByText('not_configured')).toBeInTheDocument();
  });

  test('renders missing section result for unsupported keys', () => {
    renderSection('unsupported' as never);

    expect(screen.getByText('未找到分区内容')).toBeInTheDocument();
  });
});
