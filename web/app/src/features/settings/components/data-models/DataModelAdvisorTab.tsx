import { Empty, Space, Spin, Tag, Typography } from 'antd';

import type { SettingsDataModelAdvisorFinding } from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';
import '../../../../shared/ui/structured-list/structured-list.css';

function severityColor(severity: string) {
  if (severity === 'blocking') return 'red';
  if (severity === 'high') return 'orange';
  return 'blue';
}

function advisorSeverityLabel(severity: string) {
  if (severity === 'blocking') {
    return i18nText('settings', 'auto.advisor_severity_blocking');
  }
  if (severity === 'high') {
    return i18nText('settings', 'auto.advisor_severity_high');
  }
  if (severity === 'info') {
    return i18nText('settings', 'auto.advisor_severity_info');
  }

  return severity;
}

function advisorFindingCopy(finding: SettingsDataModelAdvisorFinding) {
  if (finding.code === 'unsafe_external_source') {
    return {
      title: i18nText(
        'settings',
        'auto.advisor_finding_unsafe_external_source_title'
      ),
      message: i18nText(
        'settings',
        'auto.advisor_finding_unsafe_external_source_message'
      ),
      recommendedAction: i18nText(
        'settings',
        'auto.advisor_finding_unsafe_external_source_action'
      )
    };
  }

  if (finding.code === 'protected_model_exposure_attempt') {
    return {
      title: i18nText(
        'settings',
        'auto.advisor_finding_protected_model_exposure_attempt_title'
      ),
      message: i18nText(
        'settings',
        'auto.advisor_finding_protected_model_exposure_attempt_message'
      ),
      recommendedAction: i18nText(
        'settings',
        'auto.advisor_finding_protected_model_exposure_attempt_action'
      )
    };
  }

  if (finding.code === 'field_mapping_notice') {
    return {
      title: i18nText(
        'settings',
        'auto.advisor_finding_field_mapping_notice_title'
      ),
      message: i18nText(
        'settings',
        'auto.advisor_finding_field_mapping_notice_message'
      ),
      recommendedAction: i18nText(
        'settings',
        'auto.advisor_finding_field_mapping_notice_action'
      )
    };
  }

  return {
    title: finding.code,
    message: finding.message,
    recommendedAction: finding.recommended_action
  };
}

export function DataModelAdvisorTab({
  findings,
  loading
}: {
  findings: SettingsDataModelAdvisorFinding[];
  loading: boolean;
}) {
  if (!loading && findings.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText('settings', 'auto.no_risk_warnings')}
      />
    );
  }

  return (
    <div data-testid="data-model-advisor-tab">
      <Spin spinning={loading}>
        <ul className="structured-list__items">
          {findings.map((finding, index) => {
            const copy = advisorFindingCopy(finding);

            return (
              <li
                className="structured-list__item"
                key={`${finding.code}-${index}`}
              >
                <Space orientation="vertical" size={4}>
                  <Space wrap>
                    <Tag color={severityColor(finding.severity)}>
                      {advisorSeverityLabel(finding.severity)}
                    </Tag>
                    <Typography.Text strong>{copy.title}</Typography.Text>
                  </Space>
                  <Typography.Text>{copy.message}</Typography.Text>
                  <Typography.Text type="secondary">
                    {copy.recommendedAction}
                  </Typography.Text>
                </Space>
              </li>
            );
          })}
        </ul>
      </Spin>
    </div>
  );
}
