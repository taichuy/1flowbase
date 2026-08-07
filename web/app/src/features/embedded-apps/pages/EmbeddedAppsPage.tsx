import { Space, Typography } from 'antd';
import { i18nText } from '../../../shared/i18n/text';
import '../../../shared/ui/structured-list/structured-list.css';

const embeddedAppCapabilities = [
  i18nText('embeddedApps', 'auto.build_artifact_list'),
  i18nText('embeddedApps', 'auto.route_host_constraints'),
  i18nText('embeddedApps', 'auto.release_diagnostics_entry')
];

export function EmbeddedAppsPage() {
  return (
    <Space orientation="vertical" size="large" style={{ width: '100%' }}>
      <div>
        <Typography.Title level={2}>
          {i18nText('embeddedApps', 'auto.subsystem')}
        </Typography.Title>
        <Typography.Paragraph>
          {i18nText('embeddedApps', 'auto.subsystem_page_description')}
        </Typography.Paragraph>
      </div>
      <Typography.Paragraph>
        {i18nText('embeddedApps', 'auto.access_status_description')}
      </Typography.Paragraph>
      <ul className="structured-list__items">
        {embeddedAppCapabilities.map((item) => (
          <li className="structured-list__item" key={item}>
            {item}
          </li>
        ))}
      </ul>
    </Space>
  );
}
