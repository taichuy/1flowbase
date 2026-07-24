import { Alert, Button, Drawer, Space } from 'antd';
import { useEffect, useState } from 'react';

import { BlockSourceEditor } from '../../../../shared/code-block/BlockSourceEditor';
import { i18nText } from '../../../../shared/i18n/text';

export interface AuthenticatorUiBlockDrawerProps {
  authenticatorId: string;
  authenticatorTitle: string;
  errorMessage: string | null;
  open: boolean;
  readOnly: boolean;
  saving: boolean;
  source: string;
  onClose: () => void;
  onSave: (source: string) => Promise<void>;
}

export function AuthenticatorUiBlockDrawer({
  authenticatorId,
  authenticatorTitle,
  errorMessage,
  onClose,
  onSave,
  open,
  readOnly,
  saving,
  source
}: AuthenticatorUiBlockDrawerProps) {
  const [draft, setDraft] = useState(source);

  useEffect(() => {
    if (open) setDraft(source);
  }, [authenticatorId, open, source]);

  return (
    <Drawer
      destroyOnHidden
      open={open}
      title={i18nText('settings', 'auto.auth_center_public_ui_title', {
        value1: authenticatorTitle
      })}
      width="min(960px, calc(100vw - 24px))"
      extra={(
        <Space>
          <Button disabled={saving} onClick={onClose}>
            {i18nText('settings', 'auto.cancel')}
          </Button>
          <Button
            disabled={readOnly || draft === source}
            loading={saving}
            type="primary"
            onClick={() => void onSave(draft)}
          >
            {i18nText('settings', 'auto.save')}
          </Button>
        </Space>
      )}
      styles={{ body: { display: 'flex', flexDirection: 'column', gap: 12 } }}
      onClose={onClose}
    >
      {errorMessage ? <Alert message={errorMessage} showIcon type="error" /> : null}
      <div style={{ flex: 1, minHeight: 420 }}>
        <BlockSourceEditor
          ariaLabel={i18nText('settings', 'auto.auth_center_block_source')}
          height="100%"
          path={`file:///auth-center/${authenticatorId}/public-ui-block.tsx`}
          readOnly={readOnly || saving}
          value={draft}
          onChange={setDraft}
        />
      </div>
    </Drawer>
  );
}
