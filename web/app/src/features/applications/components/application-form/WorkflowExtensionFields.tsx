import { Form, Input, Select, Space } from 'antd';
import { useTranslation } from 'react-i18next';
import type { useApplicationFormController } from './use-application-form-controller';

type WorkflowExtensionFieldsProps = Pick<
  ReturnType<typeof useApplicationFormController>,
  'isEdit' | 'extension' | 'extensionContract'
>;

export function WorkflowExtensionFields({
  isEdit,
  extension,
  extensionContract
}: WorkflowExtensionFieldsProps) {
  const { t } = useTranslation('applications');
  return isEdit ? (
    <>
      <Form.Item label={t('auto.http_method')} htmlFor="readonly_http_method">
        <Input
          id="readonly_http_method"
          readOnly
          value={extension?.method ?? '—'}
        />
      </Form.Item>
      <Form.Item
        label={t('auto.extension_subpath')}
        htmlFor="readonly_extension_subpath"
      >
        <Input
          id="readonly_extension_subpath"
          readOnly
          value={extension ? `/api/ex/${extension.slug}` : '—'}
        />
      </Form.Item>
      <Form.Item
        label={t('auto.response_mode')}
        htmlFor="readonly_response_mode"
      >
        <Input
          id="readonly_response_mode"
          readOnly
          value={
            extension?.response_mode === 'async'
              ? t('auto.response_mode_async')
              : t('auto.response_mode_sync')
          }
        />
      </Form.Item>
      <Form.Item
        label={t('auto.request_parameters')}
        htmlFor="readonly_request_parameters"
      >
        <Input.TextArea
          id="readonly_request_parameters"
          readOnly
          rows={Math.min(
            4,
            Math.max(1, extensionContract.requestFields.length)
          )}
          value={
            extensionContract.requestFields.length > 0
              ? extensionContract.requestFields
                  .map(
                    (field) =>
                      `${field.source} · ${field.key} · ${field.valueType}`
                  )
                  .join('\n')
              : t('auto.no_request_parameters')
          }
        />
      </Form.Item>
      <Form.Item
        label={t('auto.response_fields')}
        htmlFor="readonly_response_fields"
      >
        <Input.TextArea
          id="readonly_response_fields"
          readOnly
          rows={Math.min(
            4,
            Math.max(1, extensionContract.responseFields.length)
          )}
          value={
            extensionContract.responseFields.length > 0
              ? extensionContract.responseFields
                  .map((field) => `${field.key} · ${field.valueType}`)
                  .join('\n')
              : t('auto.no_response_fields')
          }
        />
      </Form.Item>
    </>
  ) : (
    <>
      <Form.Item
        htmlFor="extension_subpath"
        label={t('auto.extension_subpath')}
        required
      >
        <Space.Compact block>
          <Input
            aria-label="/api/ex/"
            readOnly
            value="/api/ex/"
            style={{ width: 88 }}
          />
          <Form.Item
            name="extension_subpath"
            noStyle
            rules={[
              {
                required: true,
                message: t('auto.extension_subpath_required')
              }
            ]}
          >
            <Input id="extension_subpath" placeholder="orders/create" />
          </Form.Item>
        </Space.Compact>
      </Form.Item>
      <Form.Item label={t('auto.http_method')} name="extension_http_method">
        <Select
          options={['GET', 'POST', 'PUT', 'PATCH', 'DELETE'].map((value) => ({
            value,
            label: value
          }))}
        />
      </Form.Item>
      <Form.Item label={t('auto.response_mode')} name="extension_response_mode">
        <Select
          options={[
            { value: 'sync', label: t('auto.response_mode_sync') },
            { value: 'async', label: t('auto.response_mode_async') }
          ]}
        />
      </Form.Item>
    </>
  );
}
