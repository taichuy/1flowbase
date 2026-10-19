import { Result } from 'antd';
import { i18nText } from '../../../../../shared/i18n/text';

export function SettingsEmptyState() {
  return <Result status="info" title={i18nText("settings", "auto.account_access_content")} />;
}
