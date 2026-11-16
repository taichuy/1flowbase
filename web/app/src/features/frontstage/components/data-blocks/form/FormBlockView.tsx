import {
  Alert,
  Button,
  Divider,
  Empty,
  Form,
  Grid,
  Space,
  Spin,
  Typography
} from 'antd';
import XMarkdown from '@ant-design/x-markdown';
import { i18nText } from '../../../../../shared/i18n/text';
import { dataBlockError } from '../../../lib/data-blocks/fields';
import type { DataBlockHost } from '../../../lib/data-blocks/host-context';
import type {
  DataBlockComponentProps,
  DataFormItem,
  FormBlockConfig
} from '../../../lib/data-blocks/types';
import { FormDesignControls, type DesignDialog } from './FormDesignControls';
import { FormFieldControl } from './FormFieldControl';
import { FormLayout } from './FormLayout';
import { fieldState, newFormItem } from './form-state';
import type { useFormRecordSession } from './use-form-record-session';

type Props = DataBlockComponentProps & {
  config: FormBlockConfig;
  host: DataBlockHost | null;
  session: ReturnType<typeof useFormRecordSession>;
  designError: string;
  savingDesign: boolean;
  saveConfig(config: FormBlockConfig): Promise<boolean>;
  dialog: DesignDialog;
  setDialog(dialog: DesignDialog): void;
};

export function FormBlockView({
  ctx,
  config,
  host,
  session,
  designError,
  savingDesign,
  saveConfig,
  dialog,
  setDialog
}: Props) {
  const {
    metadata,
    record,
    recordId,
    fields,
    fieldsByCode,
    values,
    submitError,
    outputError,
    success,
    submitting,
    missingParent,
    missingRecord,
    missingPrimaryKey,
    canWrite,
    canRead,
    ready,
    submit
  } = session;
  const screens = Grid.useBreakpoint();
  const mobile = screens.md === false;
  const labelLayout = mobile ? 'vertical' : config.label_layout;
  const design = host?.design_mode ?? false;
  const renderItem = (item: DataFormItem) => (
    <FormItem
      item={item}
      ctx={ctx}
      config={config}
      host={host}
      session={session}
    />
  );
  const visibleItems = config.items.filter((item) => {
    if (design || item.kind !== 'field') return true;
    const field = fieldsByCode.get(item.field_code);
    return Boolean(field && fieldState(field, item, config, values).visible);
  });
  const busy = savingDesign || Boolean(host?.saving);
  const submitAction =
    config.mode !== 'view' && config.submit_enabled ? (
      <Button
        type="primary"
        loading={submitting}
        disabled={!ready || !canWrite || missingParent || busy}
        onClick={() => void submit()}
      >
        {config.submit_title ||
          i18nText('frontstage', 'data_blocks.form.submit')}
      </Button>
    ) : null;
  return (
    <section
      className={`data-form-block${design ? ' is-design' : ''}`}
      aria-label={
        config.title || i18nText('frontstage', 'data_blocks.form.form')
      }
    >
      {config.title && (
        <Typography.Title level={5}>{config.title}</Typography.Title>
      )}
      {config.description && (
        <Typography.Paragraph type="secondary">
          {config.description}
        </Typography.Paragraph>
      )}
      {metadata.isLoading && <Spin />}
      {metadata.isError && (
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.metadata_failed')}
          description={dataBlockError(metadata.error)}
        />
      )}
      {missingRecord && (
        <Alert
          type="info"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.missing_record')}
        />
      )}
      {missingPrimaryKey && (
        <Alert
          type="warning"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.missing_primary_key')}
        />
      )}
      {missingParent && (
        <Alert
          type="info"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.missing_parent')}
        />
      )}
      {metadata.data && !canRead && (
        <Alert
          type="warning"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.read_denied')}
        />
      )}
      {recordId && record.isLoading && <Spin />}
      {record.isError && (
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.record_failed')}
          description={dataBlockError(record.error)}
        />
      )}
      {designError && (
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.design_failed')}
          description={designError}
        />
      )}
      {submitError && (
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.submit_failed')}
          description={submitError}
        />
      )}
      {outputError && (
        <Alert
          type="warning"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.saved_output_failed')}
        />
      )}
      {success && (
        <Alert
          type="success"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.saved')}
        />
      )}
      {(ready || design) && (
        <Form
          component="div"
          layout={labelLayout}
          labelAlign={mobile ? 'left' : config.label_align}
          labelWrap={config.label_wrap}
          colon={config.colon}
          labelCol={
            labelLayout === 'horizontal'
              ? { flex: `${config.label_width}px` }
              : undefined
          }
          wrapperCol={labelLayout === 'horizontal' ? { flex: 1 } : undefined}
        >
          {!config.items.length && design && (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={i18nText(
                'frontstage',
                'data_blocks.form.empty_fields'
              )}
            />
          )}
          <FormLayout
            items={visibleItems}
            design={design}
            saving={busy}
            renderItem={renderItem}
            onSettings={setDialog}
            onItemsChange={(items) => {
              void saveConfig({ ...config, items });
            }}
          />
          {design && (
            <FormDesignControls
              submitAction={submitAction}
              ctx={ctx}
              config={config}
              fields={fields}
              saving={busy}
              onSave={saveConfig}
              dialog={dialog}
              setDialog={setDialog}
              onFieldCreated={async (code) => {
                await metadata.refetch();
                if (!config.items.some((item) => item.field_code === code)) {
                  const attached = await saveConfig({
                    ...config,
                    items: [...config.items, newFormItem('field', code)]
                  });
                  if (!attached)
                    throw new Error(
                      i18nText(
                        'frontstage',
                        'data_blocks.fields.created_attach_failed'
                      )
                    );
                }
              }}
            />
          )}
          {!design && submitAction && (
            <Space className="data-form-actions">{submitAction}</Space>
          )}
          {config.mode !== 'view' && metadata.data && !canWrite && (
            <Typography.Paragraph type="secondary">
              {i18nText('frontstage', 'data_blocks.form.write_denied')}
            </Typography.Paragraph>
          )}
        </Form>
      )}
    </section>
  );
}

function FormItem({
  item,
  ctx,
  config,
  host,
  session
}: Pick<Props, 'ctx' | 'config' | 'host' | 'session'> & {
  item: DataFormItem;
}) {
  const { fieldsByCode, values, errors, canWrite, submitting, changeValue } =
    session;
  const design = host?.design_mode ?? false;
  if (item.kind === 'group')
    return (
      <>
        <Divider titlePlacement="left">{item.title}</Divider>
        {item.description && (
          <Typography.Paragraph type="secondary">
            {item.description}
          </Typography.Paragraph>
        )}
      </>
    );
  if (item.kind === 'markdown')
    return (
      <XMarkdown content={item.text ?? ''} escapeRawHtml openLinksInNewTab />
    );
  const field = fieldsByCode.get(item.field_code);
  if (!field)
    return design ? (
      <Typography.Text type="secondary">
        {i18nText('frontstage', 'data_blocks.form.field_unavailable')}
      </Typography.Text>
    ) : null;
  const state = fieldState(field, item, config, values);
  const id = `form-${host?.block_id ?? config.binding.model_id}-${item.id}`;
  return (
    <Form.Item
      label={item.title ?? field.title}
      htmlFor={id}
      required={state.required}
      tooltip={item.tooltip}
      extra={item.description || undefined}
      validateStatus={errors[field.code] ? 'error' : undefined}
      help={errors[field.code]}
    >
      <FormFieldControl
        id={id}
        ctx={ctx}
        field={field}
        value={values[field.code]}
        readOnly={
          state.readOnly ||
          !canWrite ||
          submitting ||
          (config.mode === 'create' &&
            config.relation?.field_code === field.code)
        }
        onChange={(value) => changeValue(field.code, value)}
      />
    </Form.Item>
  );
}
