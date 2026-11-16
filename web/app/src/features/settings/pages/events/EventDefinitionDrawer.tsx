import type {
  CreateEventDefinitionInput,
  EventCatalogEntry
} from '@1flowbase/api-client';
import { Alert, Drawer } from 'antd';
import { useTranslation } from 'react-i18next';
import { EventDefinitionDetails } from './definitions/EventDefinitionDetails';
import { EventDefinitionForm } from './definitions/EventDefinitionForm';

export function EventDefinitionDrawer({
  entry,
  saving,
  editable,
  error,
  onClose,
  onSave
}: {
  entry: EventCatalogEntry | null;
  saving: boolean;
  editable: boolean;
  error: boolean;
  onClose: () => void;
  onSave: (values: CreateEventDefinitionInput) => void;
}) {
  const { t } = useTranslation('settingsEvents');
  return (
    <Drawer
      open
      destroyOnHidden
      size="min(760px, 100vw)"
      title={entry ? entry.name : t('create')}
      onClose={onClose}
    >
      {error && <Alert type="error" showIcon title={t('save_error')} />}
      {editable ? (
        <EventDefinitionForm
          entry={entry}
          saving={saving}
          onSave={onSave}
          onClose={onClose}
        />
      ) : entry ? (
        <EventDefinitionDetails entry={entry} />
      ) : null}
    </Drawer>
  );
}
