import { useState } from 'react';
import {
  Alert,
  Checkbox,
  DatePicker,
  Input,
  InputNumber,
  Pagination,
  Radio,
  Select,
  Space,
  Switch,
  Typography
} from 'antd';
import dayjs from 'dayjs';
import type { BlockContext } from '@1flowbase/page-protocol';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  useDataBlockMetadata,
  useDataBlockRecords
} from '../../../api/data-blocks';
import {
  dataFieldOptions,
  dataFieldText,
  isMultipleDataField
} from '../../../lib/data-blocks/fields';
import type { DataBlockField } from '../../../lib/data-blocks/types';

type Props = {
  ctx: BlockContext;
  field: DataBlockField;
  value: unknown;
  onChange(value: unknown): void;
  readOnly?: boolean;
  id?: string;
};
const popupContainer = (node: HTMLElement) => node.parentElement ?? node;

function RelationControl({ ctx, field, value, onChange, readOnly, id }: Props) {
  const [page, setPage] = useState(1);
  const [labels, setLabels] = useState<Record<string, string>>({});
  const binding =
    field.relation_target_model_code && field.relation_target_model_id
      ? {
          model_code: field.relation_target_model_code,
          model_id: field.relation_target_model_id,
          data_source_id: null
        }
      : undefined;
  const metadata = useDataBlockMetadata(ctx, binding);
  const records = useDataBlockRecords(
    ctx,
    binding,
    { page, page_size: 20 },
    Boolean(binding && metadata.data?.record_capabilities.can_list)
  );
  const idField = metadata.data?.record_id_field;
  const labelField = metadata.data?.fields.find(
    (candidate) =>
      candidate.field_kind === 'string' && candidate.code !== idField
  );
  const options = (records.data?.items ?? []).flatMap((record) => {
    const recordId = idField ? record[idField] : undefined;
    if (typeof recordId !== 'string' && typeof recordId !== 'number') return [];
    const title = labelField
      ? dataFieldText(labelField, record[labelField.code])
      : '';
    return [
      {
        value: recordId,
        label: title ? `${title} (${recordId})` : String(recordId)
      }
    ];
  });
  const selected = (
    Array.isArray(value) ? value : value == null ? [] : [value]
  ).filter(
    (item): item is string | number =>
      typeof item === 'string' || typeof item === 'number'
  );
  const merged = [
    ...options,
    ...selected
      .filter(
        (recordId) => !options.some((option) => option.value === recordId)
      )
      .map((recordId) => ({
        value: recordId,
        label: labels[String(recordId)] ?? String(recordId)
      }))
  ];
  if (readOnly)
    return (
      <Typography.Text>
        {selected
          .map(
            (recordId) =>
              merged.find((option) => option.value === recordId)?.label ??
              String(recordId)
          )
          .join(', ') || '—'}
      </Typography.Text>
    );
  return (
    <Space direction="vertical" style={{ width: '100%' }}>
      <Select
        id={id}
        style={{ width: '100%' }}
        allowClear
        disabled={
          !binding ||
          !idField ||
          metadata.data?.record_capabilities.can_list === false
        }
        loading={metadata.isLoading || records.isLoading}
        mode={field.field_kind === 'many_to_many' ? 'multiple' : undefined}
        options={merged}
        value={field.field_kind === 'many_to_many' ? selected : selected[0]}
        getPopupContainer={popupContainer}
        onChange={(next) => {
          setLabels((previous) => ({
            ...previous,
            ...Object.fromEntries(
              merged.map((option) => [String(option.value), option.label])
            )
          }));
          onChange(next ?? (field.field_kind === 'many_to_many' ? [] : null));
        }}
        popupRender={(menu) => (
          <>
            {menu}
            <div
              className="data-form-relation-pagination"
              onMouseDown={(event) => event.preventDefault()}
            >
              <Pagination
                size="small"
                current={page}
                pageSize={20}
                total={records.data?.total ?? 0}
                onChange={setPage}
                showSizeChanger={false}
              />
            </div>
          </>
        )}
      />
      {(!binding || (metadata.data && !idField)) && (
        <Typography.Text type="secondary">
          {i18nText('frontstage', 'data_blocks.form.relation_unavailable')}
        </Typography.Text>
      )}
      {(metadata.isError || records.isError) && (
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.relation_failed')}
        />
      )}
    </Space>
  );
}

export function FormFieldControl(props: Props) {
  const { field, value, onChange, readOnly, id } = props;
  if (field.field_kind === 'many_to_one' || field.field_kind === 'many_to_many')
    return <RelationControl {...props} />;
  if (
    readOnly &&
    (field.field_kind === 'json' || field.display_interface === 'json')
  ) {
    return (
      <Input.TextArea
        id={id}
        readOnly
        autoSize={{ minRows: 2, maxRows: 10 }}
        value={
          value == null
            ? ''
            : typeof value === 'string'
              ? value
              : JSON.stringify(value, null, 2)
        }
      />
    );
  }
  if (readOnly)
    return (
      <Typography.Text className="data-form-readonly">
        {dataFieldText(field, value) || '—'}
      </Typography.Text>
    );
  const display = field.display_interface ?? field.field_kind;
  const options = dataFieldOptions(field);
  if (options.length || field.field_kind === 'enum') {
    const values = Array.isArray(value) ? value : [value];
    const selected = options.flatMap((option, index) =>
      values.includes(option.value) ? [index] : []
    );
    const indexedOptions = options.map((option, index) => ({
      label: option.label,
      value: index
    }));
    if (display === 'radio' || display === 'radio_group')
      return (
        <Radio.Group
          id={id}
          options={indexedOptions}
          value={selected[0]}
          onChange={(event) => onChange(options[event.target.value]?.value)}
        />
      );
    if (display === 'checkbox' || display === 'checkbox_group')
      return (
        <Checkbox.Group
          id={id}
          options={indexedOptions}
          value={selected}
          onChange={(next) =>
            onChange(next.map((index) => options[Number(index)].value))
          }
        />
      );
    const multiple = isMultipleDataField(field);
    return (
      <Select
        id={id}
        style={{ width: '100%' }}
        getPopupContainer={popupContainer}
        allowClear
        options={indexedOptions}
        mode={multiple ? 'multiple' : undefined}
        value={multiple ? selected : selected[0]}
        onChange={(next) =>
          onChange(
            Array.isArray(next)
              ? next.map((index) => options[index].value)
              : next == null
                ? null
                : options[next]?.value
          )
        }
      />
    );
  }
  if (field.field_kind === 'boolean' || display === 'switch')
    return display === 'checkbox' ? (
      <Checkbox
        id={id}
        checked={value === true}
        onChange={(event) => onChange(event.target.checked)}
      />
    ) : (
      <Switch id={id} checked={value === true} onChange={onChange} />
    );
  if (['number', 'integer', 'decimal', 'float'].includes(field.field_kind))
    return (
      <InputNumber
        id={id}
        style={{ width: '100%' }}
        value={typeof value === 'number' ? value : null}
        onChange={onChange}
      />
    );
  if (['datetime', 'date', 'timestamp'].includes(field.field_kind))
    return (
      <DatePicker
        id={id}
        style={{ width: '100%' }}
        getPopupContainer={popupContainer}
        showTime={display !== 'date'}
        value={
          typeof value === 'string' && dayjs(value).isValid()
            ? dayjs(value)
            : null
        }
        onChange={(next) =>
          onChange(
            next
              ? display === 'date'
                ? next.format('YYYY-MM-DD')
                : next.toISOString()
              : null
          )
        }
      />
    );
  if (field.field_kind === 'json' || display === 'json')
    return (
      <Input.TextArea
        id={id}
        rows={4}
        value={
          typeof value === 'string'
            ? value
            : value == null
              ? ''
              : JSON.stringify(value, null, 2)
        }
        onChange={(event) => onChange(event.target.value)}
      />
    );
  if (
    field.field_kind === 'text' ||
    display === 'textarea' ||
    display === 'text'
  )
    return (
      <Input.TextArea
        id={id}
        autoSize={{ minRows: 3, maxRows: 10 }}
        value={value == null ? '' : String(value)}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  return (
    <Input
      id={id}
      value={value == null ? '' : String(value)}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}
