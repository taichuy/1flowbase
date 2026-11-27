import { Alert, Button } from 'antd';
import { i18nText } from '../../../../../shared/i18n/text';

export function DirectoryReadError({ error, retry }: { error: Error; retry: () => void }) {
  return (
    <Alert
      type="error"
      title={i18nText('settings', 'auto.mcp_load_failed')}
      description={error.message}
      action={
        <Button onMouseDown={(event) => event.preventDefault()} onClick={retry}>
          {i18nText('settings', 'auto.retry')}
        </Button>
      }
    />
  );
}
