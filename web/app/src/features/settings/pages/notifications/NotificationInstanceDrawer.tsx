import { useMemo, useRef, useState } from 'react';
import { Alert, Form, Input, Select, Tag } from 'antd';
import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import MessageOutlined from '@ant-design/icons/es/icons/MessageOutlined';
import { useTranslation } from 'react-i18next';
import type {
  NotificationInstance,
  NotificationType
} from '@1flowbase/api-client';
import {
  SchemaFormDrawer,
  type SchemaFormValues
} from '../../../../shared/schema-ui/v1/form-drawer/SchemaFormDrawer';
import type { PluginFormSchema } from '../../../../shared/schema-ui/v1/contracts/plugin-form-schema';
import {
  createNotificationInstance,
  updateNotificationInstance
} from '../../../notifications/api/queries';
import { NotificationAuthorization } from './NotificationAuthorization';
import { useAuthStore } from '../../../../state/auth-store';
import { useNotificationTypeLocalization } from '../../../notifications/hooks/use-notification-type-localization';
import './notification-instance-drawer.css';
import {
  CommunicationBindingForm,
  type CommunicationBindingHandle
} from './communications/CommunicationBindingForm';

export function NotificationInstanceDrawer({
  instance,
  types,
  onClose,
  onSaved
}: {
  instance?: NotificationInstance;
  types: NotificationType[];
  onClose: () => void;
  onSaved: (options?: { keepOpen: boolean }) => void;
}) {
  const { t } = useTranslation('notifications');
  const localizeType = useNotificationTypeLocalization();
  const [savedInstance, setSavedInstance] = useState(instance);
  const [selectedIdentity, setSelectedIdentity] = useState<string>();
  const typeIdentity = savedInstance
    ? JSON.stringify([savedInstance.type_code, savedInstance.installation_id])
    : selectedIdentity;
  const type = useMemo(() => {
    const selected = types.find(
      (item) =>
        JSON.stringify([item.type_code, item.installation_id]) === typeIdentity
    );
    return selected && localizeType(selected);
  }, [types, typeIdentity, localizeType]);
  const createdInSubmit = useRef(false);
  const bindingForm = useRef<CommunicationBindingHandle>(null);
  const draftConfig = useRef<SchemaFormValues | undefined>(undefined);
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const [beginOnMount, setBeginOnMount] = useState(false);
  const [display_name, setDisplayName] = useState(instance?.display_name ?? '');
  const [nameError, setNameError] = useState(false);
  const schema = useMemo<PluginFormSchema>(() => {
    if (!type) return { schema_version: '1.0.0', fields: [] };
    // Canonical wire schema is the same PluginFormSchema consumed by schema UI.
    const form = type.config_form as PluginFormSchema;
    return {
      ...form,
      fields: form.fields.map((field) =>
        savedInstance?.configured_secret_fields.includes(field.key)
          ? {
              ...field,
              required: false,
              default_value: undefined,
              placeholder: t('secret_preserved')
            }
          : field
      )
    };
  }, [savedInstance, type, t]);
  const initialValuesScope = useRef<{
    schema: PluginFormSchema;
    instance: NotificationInstance | undefined;
    values: SchemaFormValues;
  } | null>(null);
  if (
    !initialValuesScope.current ||
    initialValuesScope.current.schema !== schema ||
    initialValuesScope.current.instance !== savedInstance
  ) {
    initialValuesScope.current = {
      schema,
      instance: savedInstance,
      values: {
        // Refresh the schema baseline on locale/type changes without losing the live draft.
        ...(savedInstance?.config_json as SchemaFormValues | undefined),
        ...draftConfig.current
      }
    };
  }
  const initialValues = initialValuesScope.current.values;
  const unavailable = !!savedInstance && !savedInstance.available;
  const unavailableTitle =
    savedInstance?.unavailable_reason === 'instance_disabled'
      ? t('instance_disabled')
      : savedInstance?.unavailable_reason === 'plugin_unavailable'
        ? t('plugin_unavailable')
        : t('type_unavailable');
  return (
    <SchemaFormDrawer
      key={typeIdentity ?? 'new'}
      open
      title={savedInstance ? t('edit_instance') : t('create_instance')}
      rootClassName="notification-instance-drawer"
      footerAlign="end"
      actionsPlacement="footer"
      bodyClassName="notification-instance-drawer__body"
      extra={
        savedInstance && (
          <Tag
            className="notification-status"
            color={savedInstance.status === 'enabled' ? 'success' : 'default'}
            icon={
              savedInstance.status === 'enabled' ? <CheckOutlined /> : undefined
            }
          >
            {savedInstance.status === 'enabled' ? t('enabled') : t('disabled')}
          </Tag>
        )
      }
      schema={schema}
      initialValues={initialValues}
      resizable
      defaultWidth={720}
      minWidth={480}
      maxWidth={1200}
      resizeLabel={t('resize_drawer')}
      submitText={
        !savedInstance && type?.authorization
          ? t('save_and_authorize')
          : savedInstance
            ? t('save_changes')
            : t('submit')
      }
      cancelText={t('cancel')}
      leadingContent={
        <div className="notification-instance-drawer__sections">
          <Form layout="vertical" component={false}>
            <Form.Item
              label={t('display_name')}
              required
              validateStatus={nameError ? 'error' : undefined}
              help={nameError ? t('name_required') : undefined}
            >
              <Input
                className="notification-instance-drawer__name"
                size="large"
                autoFocus
                value={display_name}
                onChange={(event) => {
                  setDisplayName(event.target.value);
                  setNameError(false);
                }}
                aria-label={t('display_name')}
                autoComplete="off"
              />
            </Form.Item>
            <Form.Item label={t('type')} required={!savedInstance}>
              {savedInstance ? (
                <Input
                  className="notification-instance-drawer__type"
                  size="large"
                  prefix={<MessageOutlined aria-hidden />}
                  readOnly
                  value={type?.display_name ?? savedInstance.type_code}
                  aria-label={t('type')}
                />
              ) : (
                <Select
                  className="notification-instance-drawer__type"
                  size="large"
                  aria-label={t('type')}
                  value={selectedIdentity}
                  options={types
                    .filter((item) => item.available)
                    .map((item) => ({
                      value: JSON.stringify([
                        item.type_code,
                        item.installation_id
                      ]),
                      label: localizeType(item).display_name
                    }))}
                  onChange={(value: string) => {
                    draftConfig.current = undefined;
                    setSelectedIdentity(value);
                  }}
                />
              )}
            </Form.Item>
          </Form>
          {type?.receive && (
            <CommunicationBindingForm
              ref={bindingForm}
              instance_id={savedInstance?.id}
              receive={type.receive}
            />
          )}
          {type?.authorization && savedInstance?.authorization && (
            <NotificationAuthorization
              display_name={type.display_name}
              description={type.description ?? undefined}
              instance_id={savedInstance.id}
              descriptor={type.authorization}
              authorization={savedInstance.authorization}
              beginOnMount={beginOnMount}
              onChanged={() => onSaved({ keepOpen: true })}
            />
          )}
          {unavailable && <Alert type="warning" title={unavailableTitle} />}
          {!type && instance && (
            <Alert type="info" title={t('config_unavailable')} />
          )}
        </div>
      }
      submitDisabled={
        !display_name.trim() || (!savedInstance && !type?.available)
      }
      onCancel={onClose}
      onValuesChange={(values) => {
        // A language change refreshes labels, while the open form keeps its draft.
        draftConfig.current = values;
      }}
      onBeforeSubmit={() => {
        if (!display_name.trim()) {
          setNameError(true);
          return false;
        }
        if (type?.receive && !bindingForm.current?.validate()) return false;
      }}
      onSubmit={async (values) => {
        let bindingFailed = false;
        try {
          const config_json = Object.fromEntries(
            Object.entries(values).filter(
              ([key, value]) =>
                value !== undefined &&
                !(
                  savedInstance?.configured_secret_fields.includes(key) &&
                  (value === '' || value === null)
                )
            )
          );
          let result: NotificationInstance | undefined;
          if (savedInstance) {
            result = await updateNotificationInstance(
              savedInstance.id,
              {
                display_name: display_name.trim(),
                ...(type ? { config_json } : {})
              },
              csrfToken
            );
          } else if (type?.available) {
            createdInSubmit.current = true;
            result = await createNotificationInstance(
              {
                display_name: display_name.trim(),
                type_code: type.type_code,
                installation_id: type.installation_id,
                config_json
              },
              csrfToken
            );
          }
          if (result && type?.receive) {
            // Persist the channel identity before the second write, so retry cannot duplicate it.
            setBeginOnMount(createdInSubmit.current);
            setSavedInstance(result);
            onSaved({ keepOpen: true });
            bindingFailed = true;
            await bindingForm.current?.save(result.id);
            bindingFailed = false;
          }
          return result;
        } catch {
          throw new Error(
            t(bindingFailed ? 'binding_save_failed' : 'instances_failed')
          );
        }
      }}
      onSubmitSuccess={(result) => {
        draftConfig.current = undefined;
        if (type?.authorization) {
          setBeginOnMount(createdInSubmit.current);
          setSavedInstance(result as NotificationInstance);
          onSaved({ keepOpen: true });
        } else {
          onSaved();
        }
      }}
    />
  );
}
