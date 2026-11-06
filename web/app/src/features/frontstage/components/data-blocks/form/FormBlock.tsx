import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Alert,
  Button,
  Divider,
  Empty,
  Form,
  Space,
  Spin,
  Typography
} from 'antd';
import XMarkdown from '@ant-design/x-markdown';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  useDataBlockMetadata,
  useDataBlockRecord,
  useDataBlockWrites
} from '../../../api/data-blocks';
import { dataBlockError } from '../../../lib/data-blocks/fields';
import { relationRecordId } from '../../../lib/data-blocks/filters';
import { useDataBlockHost } from '../../../lib/data-blocks/host-context';
import {
  readDataBlockConfig,
  type DataBlockComponentProps,
  type DataFormItem,
  type FormBlockConfig
} from '../../../lib/data-blocks/types';
import {
  FormDesignControls,
  newFormItem,
  type DesignDialog
} from './FormDesignControls';
import { FormFieldControl } from './FormFieldControl';
import { FormLayout } from './FormLayout';
import {
  defaultFormValues,
  fieldState,
  formRecordId,
  formWriteValues,
  isEmptyFormValue,
  recordIdentity
} from './form-state';

export function FormBlock({ ctx }: DataBlockComponentProps) {
  const config = readDataBlockConfig(ctx.props);
  const host = useDataBlockHost();
  if (!config || config.kind !== 'form')
    return (
      <Alert
        type="warning"
        showIcon
        title={i18nText('frontstage', 'data_blocks.form.not_configured')}
      />
    );
  return (
    <FormBlockSession
      key={`${ctx.workspace.id}:${ctx.currentUser?.id ?? ''}:${host?.block_id ?? ''}:${config.binding.model_id}:${recordIdentity(config, ctx.inputs)}`}
      ctx={ctx}
      initialConfig={config}
    />
  );
}

function FormBlockSession({
  ctx,
  initialConfig
}: DataBlockComponentProps & { initialConfig: FormBlockConfig }) {
  const host = useDataBlockHost();
  const [config, setConfig] = useState(initialConfig);
  const [values, setValues] = useState<Record<string, unknown>>(() =>
    initialConfig.mode === 'create'
      ? defaultFormValues(initialConfig.items)
      : {}
  );
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [submitError, setSubmitError] = useState('');
  const [designError, setDesignError] = useState('');
  const [success, setSuccess] = useState(false);
  const [outputError, setOutputError] = useState(false);
  const [savingDesign, setSavingDesign] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [dialog, setDialog] = useState<DesignDialog>(null);
  const initialized = useRef(false);
  const knownFields = useRef(new Set<string>());
  const active = useRef(true);
  const writing = useRef(false);
  const saving = useRef(false);
  const revision = useRef(0);
  const metadata = useDataBlockMetadata(ctx, config.binding);
  const recordId = config.mode === 'create' ? null : formRecordId(ctx.inputs);
  const record = useDataBlockRecord(
    ctx,
    config.binding,
    recordId,
    config.mode !== 'create' &&
      Boolean(
        metadata.data?.record_capabilities.can_get &&
        metadata.data.record_id_field
      )
  );
  const writes = useDataBlockWrites(
    ctx,
    config.binding,
    config.source_model_code
  );
  const fields = metadata.data?.fields ?? [];
  const design = host?.design_mode ?? false;
  const parentId = relationRecordId(config.relation, ctx.inputs);
  const missingParent =
    config.mode === 'create' && Boolean(config.relation) && !parentId;
  const missingRecord = config.mode !== 'create' && !recordId;
  const missingPrimaryKey =
    config.mode !== 'create' && metadata.data && !metadata.data.record_id_field;
  const canWrite =
    config.mode === 'create'
      ? metadata.data?.record_capabilities.can_create
      : config.mode === 'edit'
        ? metadata.data?.record_capabilities.can_update
        : false;
  const canRead =
    config.mode === 'create' || metadata.data?.record_capabilities.can_get;

  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  useEffect(() => {
    if (!saving.current) setConfig(initialConfig);
  }, [initialConfig]);
  useEffect(() => {
    if (config.mode === 'create') {
      if (!metadata.data) return;
      const defaults: Record<string, unknown> = {};
      for (const item of config.items) {
        if (!item.field_code || knownFields.current.has(item.field_code))
          continue;
        knownFields.current.add(item.field_code);
        const field = metadata.data.fields.find(
          (candidate) => candidate.code === item.field_code
        );
        const value =
          item.default_value !== undefined
            ? item.default_value
            : field?.default_value;
        if (value !== undefined && value !== null)
          defaults[item.field_code] = value;
      }
      setValues((previous) => ({
        ...defaults,
        ...previous,
        ...(config.relation && parentId
          ? { [config.relation.field_code]: parentId }
          : {})
      }));
    } else if (!initialized.current && record.data) {
      initialized.current = true;
      setValues({ ...record.data });
    }
  }, [
    config.items,
    config.mode,
    config.relation,
    parentId,
    metadata.data,
    record.data
  ]);

  const saveConfig = useCallback(
    async (next: FormBlockConfig) => {
      if (!host || !host.design_mode || saving.current) return false;
      const previous = config;
      saving.current = true;
      setSavingDesign(true);
      setDesignError('');
      setConfig(next);
      try {
        await host.saveConfig(next);
        return true;
      } catch (error) {
        if (active.current) {
          setConfig(previous);
          setDesignError(dataBlockError(error));
        }
        return false;
      } finally {
        saving.current = false;
        if (active.current) setSavingDesign(false);
      }
    },
    [config, host]
  );

  const submit = async () => {
    if (
      writing.current ||
      !canWrite ||
      missingRecord ||
      missingParent ||
      missingPrimaryKey ||
      !metadata.data ||
      (config.mode !== 'create' && !initialized.current)
    )
      return;
    setSuccess(false);
    setOutputError(false);
    setSubmitError('');
    const nextErrors: Record<string, string> = {};
    for (const item of config.items) {
      const field = fields.find(
        (candidate) => candidate.code === item.field_code
      );
      if (!field || item.kind !== 'field') continue;
      const state = fieldState(field, item, config, values);
      if (
        state.visible &&
        !state.readOnly &&
        state.required &&
        isEmptyFormValue(values[field.code]) &&
        !(
          config.mode === 'create' &&
          config.relation?.field_code === field.code &&
          parentId
        )
      )
        nextErrors[field.code] = i18nText(
          'frontstage',
          'data_blocks.form.required_error'
        );
    }
    setErrors(nextErrors);
    if (Object.keys(nextErrors).length) return;
    let payload: Record<string, unknown>;
    try {
      payload = formWriteValues(config, fields, values, ctx.inputs);
      for (const field of fields) {
        if (
          (field.field_kind === 'json' || field.display_interface === 'json') &&
          typeof payload[field.code] === 'string'
        ) {
          try {
            payload[field.code] =
              payload[field.code] === ''
                ? null
                : JSON.parse(payload[field.code] as string);
          } catch {
            setErrors((previous) => ({
              ...previous,
              [field.code]: i18nText(
                'frontstage',
                'data_blocks.form.invalid_json'
              )
            }));
            return;
          }
        }
      }
    } catch {
      setSubmitError(
        i18nText('frontstage', 'data_blocks.form.invalid_relation')
      );
      return;
    }
    writing.current = true;
    setSubmitting(true);
    try {
      const saved =
        config.mode === 'create'
          ? await writes.create.mutateAsync(payload)
          : await writes.update.mutateAsync({
              record_id: recordId!,
              values: payload
            });
      if (!active.current) return;
      revision.current += 1;
      const idField = metadata.data.record_id_field;
      const savedId = idField ? saved[idField] : null;
      setSuccess(true);
      try {
        const published = await ctx.outputs.publish({
          ...(typeof savedId === 'string' || typeof savedId === 'number'
            ? { saved_record_id: String(savedId) }
            : {}),
          saved_record: saved,
          save_revision: revision.current
        });
        if (!active.current) return;
        if (!published.ok || published.stale) {
          setOutputError(true);
          return;
        }
      } catch {
        if (active.current) setOutputError(true);
        return;
      }
      if (config.close_after_submit) host?.close();
    } catch (error) {
      if (active.current) setSubmitError(dataBlockError(error));
    } finally {
      writing.current = false;
      if (active.current) setSubmitting(false);
    }
  };

  const renderItem = (item: DataFormItem) => {
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
    const field = fields.find(
      (candidate) => candidate.code === item.field_code
    );
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
          onChange={(value) => {
            setValues((previous) => ({ ...previous, [field.code]: value }));
            setErrors((previous) => {
              const next = { ...previous };
              delete next[field.code];
              return next;
            });
            setSuccess(false);
          }}
        />
      </Form.Item>
    );
  };
  const visibleItems = config.items.filter((item) => {
    if (design || item.kind !== 'field') return true;
    const field = fields.find(
      (candidate) => candidate.code === item.field_code
    );
    return Boolean(field && fieldState(field, item, config, values).visible);
  });
  const busy = savingDesign || Boolean(host?.saving);
  const ready = Boolean(
    metadata.data &&
    canRead &&
    !missingRecord &&
    !missingPrimaryKey &&
    (config.mode === 'create' || (record.data && !record.isError))
  );
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
          layout={config.label_layout}
          labelAlign={config.label_align}
          labelWrap={config.label_wrap}
          colon={config.colon}
          labelCol={
            config.label_layout === 'horizontal'
              ? { flex: `${config.label_width}px` }
              : undefined
          }
          wrapperCol={
            config.label_layout === 'horizontal' ? { flex: 1 } : undefined
          }
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
