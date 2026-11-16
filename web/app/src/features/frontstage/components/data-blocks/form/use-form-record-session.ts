import { useEffect, useRef, useState, type RefObject } from 'react';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  useDataBlockMetadata,
  useDataBlockRecord,
  useDataBlockWrites
} from '../../../api/data-blocks';
import { i18nText } from '../../../../../shared/i18n/text';
import { dataBlockError } from '../../../lib/data-blocks/fields';
import { relationRecordId } from '../../../lib/data-blocks/filters';
import type { DataBlockHost } from '../../../lib/data-blocks/host-context';
import type {
  DataBlockField,
  FormBlockConfig
} from '../../../lib/data-blocks/types';
import {
  defaultFormValues,
  fieldState,
  formRecordId,
  formWriteValues,
  isEmptyFormValue
} from './form-state';

export function useFormRecordSession(
  ctx: BlockContext,
  config: FormBlockConfig,
  host: DataBlockHost | null,
  active: RefObject<boolean>
) {
  const draft = useFormRecordDraft(ctx, config);
  const mutation = useFormSubmission(ctx, config, host, active, draft);
  const changeValue = (fieldCode: string, value: unknown) => {
    draft.setValues((previous) => ({ ...previous, [fieldCode]: value }));
    mutation.setErrors((previous) => {
      const next = { ...previous };
      delete next[fieldCode];
      return next;
    });
    mutation.setSuccess(false);
  };
  return { ...draft, ...mutation, changeValue };
}

function useFormRecordDraft(ctx: BlockContext, config: FormBlockConfig) {
  const [values, setValues] = useState<Record<string, unknown>>(() =>
    config.mode === 'create' ? defaultFormValues(config.items) : {}
  );
  const initialized = useRef(false);
  const knownFields = useRef<Set<string> | null>(null);
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
  const fields = metadata.data?.fields ?? [];
  const fieldsByCode = new Map<string | undefined, DataBlockField>();
  for (const field of fields) {
    if (!fieldsByCode.has(field.code)) fieldsByCode.set(field.code, field);
  }
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
    if (config.mode === 'create') {
      if (!metadata.data) return;
      const known = (knownFields.current ??= new Set<string>());
      const defaultFieldsByCode = new Map<string, DataBlockField>();
      for (const field of metadata.data.fields) {
        if (!defaultFieldsByCode.has(field.code))
          defaultFieldsByCode.set(field.code, field);
      }
      const defaults: Record<string, unknown> = {};
      for (const item of config.items) {
        if (!item.field_code || known.has(item.field_code)) continue;
        known.add(item.field_code);
        const field = defaultFieldsByCode.get(item.field_code);
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

  const ready = Boolean(
    metadata.data &&
    canRead &&
    !missingRecord &&
    !missingPrimaryKey &&
    (config.mode === 'create' || (record.data && !record.isError))
  );
  return {
    metadata,
    record,
    recordId,
    fields,
    fieldsByCode,
    values,
    setValues,
    initialized,
    parentId,
    missingParent,
    missingRecord,
    missingPrimaryKey,
    canWrite,
    canRead,
    ready
  };
}

function useFormSubmission(
  ctx: BlockContext,
  config: FormBlockConfig,
  host: DataBlockHost | null,
  active: RefObject<boolean>,
  draft: ReturnType<typeof useFormRecordDraft>
) {
  const {
    metadata,
    recordId,
    fields,
    fieldsByCode,
    values,
    initialized,
    parentId,
    missingParent,
    missingRecord,
    missingPrimaryKey,
    canWrite
  } = draft;
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [submitError, setSubmitError] = useState('');
  const [success, setSuccess] = useState(false);
  const [outputError, setOutputError] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const writing = useRef(false);
  const revision = useRef(0);
  const writes = useDataBlockWrites(
    ctx,
    config.binding,
    config.source_model_code
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
      const field = fieldsByCode.get(item.field_code);
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

  return {
    errors,
    setErrors,
    submitError,
    success,
    setSuccess,
    outputError,
    submitting,
    submit
  };
}
