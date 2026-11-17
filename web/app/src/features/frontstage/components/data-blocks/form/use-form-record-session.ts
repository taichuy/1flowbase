import {
  useCallback,
  useRef,
  useState,
  type RefObject,
  type SetStateAction
} from 'react';
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

interface FormRecordSource {
  items: FormBlockConfig['items'];
  mode: FormBlockConfig['mode'];
  relation: FormBlockConfig['relation'];
  parentId: string | null;
  metadata: ReturnType<typeof useDataBlockMetadata>['data'];
  record: ReturnType<typeof useDataBlockRecord>['data'];
}

interface FormRecordSnapshot {
  source: FormRecordSource;
  values: Record<string, unknown>;
  knownFields: ReadonlySet<string>;
  initialized: boolean;
}

function sameFormRecordSource(previous: FormRecordSource, next: FormRecordSource) {
  return (
    previous.items === next.items &&
    previous.mode === next.mode &&
    previous.relation === next.relation &&
    previous.parentId === next.parentId &&
    previous.metadata === next.metadata &&
    previous.record === next.record
  );
}

function receiveFormRecordSource(
  previous: FormRecordSnapshot,
  source: FormRecordSource
): FormRecordSnapshot {
  const next = { ...previous, source };
  if (source.mode === 'create') {
    if (!source.metadata) return next;
    const knownFields = new Set(previous.knownFields);
    const fieldsByCode = new Map<string, DataBlockField>();
    for (const field of source.metadata.fields) {
      if (!fieldsByCode.has(field.code)) fieldsByCode.set(field.code, field);
    }
    const defaults: Record<string, unknown> = {};
    for (const item of source.items) {
      if (!item.field_code || knownFields.has(item.field_code)) continue;
      knownFields.add(item.field_code);
      const field = fieldsByCode.get(item.field_code);
      const value =
        item.default_value !== undefined
          ? item.default_value
          : field?.default_value;
      if (value !== undefined && value !== null)
        defaults[item.field_code] = value;
    }
    return {
      ...next,
      knownFields,
      values: {
        ...defaults,
        ...previous.values,
        ...(source.relation && source.parentId
          ? { [source.relation.field_code]: source.parentId }
          : {})
      }
    };
  }
  if (!previous.initialized && source.record) {
    return { ...next, initialized: true, values: { ...source.record } };
  }
  return next;
}

function useFormRecordDraft(ctx: BlockContext, config: FormBlockConfig) {
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
  const parentId = relationRecordId(config.relation, ctx.inputs);
  const source: FormRecordSource = {
    items: config.items,
    mode: config.mode,
    relation: config.relation,
    parentId,
    metadata: metadata.data,
    record: record.data
  };
  const [snapshot, setSnapshot] = useState<FormRecordSnapshot>(() =>
    receiveFormRecordSource(
      {
        source,
        values: config.mode === 'create' ? defaultFormValues(config.items) : {},
        knownFields: new Set(),
        initialized: false
      },
      source
    )
  );
  if (!sameFormRecordSource(snapshot.source, source)) {
    // Update only this owner before its children commit a stale source value.
    setSnapshot(receiveFormRecordSource(snapshot, source));
  }
  const setValues = useCallback(
    (update: SetStateAction<Record<string, unknown>>) => {
      setSnapshot((previous) => {
        const values =
          typeof update === 'function' ? update(previous.values) : update;
        return values === previous.values ? previous : { ...previous, values };
      });
    },
    []
  );
  const { values, initialized } = snapshot;
  const fields = metadata.data?.fields ?? [];
  const fieldsByCode = new Map<string | undefined, DataBlockField>();
  for (const field of fields) {
    if (!fieldsByCode.has(field.code)) fieldsByCode.set(field.code, field);
  }
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
      (config.mode !== 'create' && !initialized)
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
