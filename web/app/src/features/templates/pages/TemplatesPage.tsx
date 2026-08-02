import { Typography } from 'antd';
import { useTranslation } from 'react-i18next';

import { AgentFlowTemplateLibrary } from '../components/AgentFlowTemplateLibrary';
import './templates-page.css';

export function TemplatesPage() {
  const { t } = useTranslation('templates');

  return (
    <div className="templates-page">
      <div className="templates-page__title">
        <Typography.Title level={2}>{t('auto.templates')}</Typography.Title>
        <Typography.Paragraph type="secondary">
          {t('auto.official_agent_flow_templates_description')}
        </Typography.Paragraph>
      </div>
      <AgentFlowTemplateLibrary />
    </div>
  );
}
