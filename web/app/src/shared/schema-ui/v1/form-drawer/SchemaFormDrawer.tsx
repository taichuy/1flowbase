import { Alert, Button, Drawer, Flex, Form, Input, InputNumber, Modal, Select, Space, Switch } from 'antd';
import type { Rule } from 'antd/es/form';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import { i18nText } from '../../../i18n/text';
import { ResizableDrawer } from '../../../ui/resizable-drawer/ResizableDrawer';
import type {
  PluginFormFieldSchema,
  PluginFormSchema,
  PluginFormValue
} from '../contracts/plugin-form-schema';

import './schema-form-drawer.css';

export type SchemaFormValues = Record<string, PluginFormValue | undefined>;

export interface SchemaFormDrawerContext {
  validate: () => Promise<boolean>;
  submit: () => Promise<void>;
  reset: () => void;
  close: () => void;
  getValues: () => SchemaFormValues;
  setValues: (values: SchemaFormValues) => void;
  setFieldValue: (name: string, value: PluginFormValue | undefined) => void;
}

export interface SchemaFormDrawerAction {
  key: string;
  label: string;
  icon?: ReactNode;
  placement?: 'left' | 'right';
  variant?: 'default' | 'danger';
  disabled?: boolean;
  loading?: boolean;
  onClick: (context: SchemaFormDrawerContext) => Promise<void> | void;
}

export interface SchemaFormDrawerStatusMessage {
  key: string;
  message: ReactNode;
  type: 'error' | 'info' | 'success' | 'warning';
}

export interface SchemaFormDrawerProps {
  open: boolean;
  title: string;
  subtitle?: string;
  schema: PluginFormSchema;
  initialValues?: SchemaFormValues;
  bodyClassName?: string;
  defaultWidth?: number;
  disabled?: boolean;
  leadingContent?: ReactNode;
  maxWidth?: number;
  minWidth?: number;
  resizable?: boolean;
  resizeLabel?: string;
  rootClassName?: string;
  statusMessages?: SchemaFormDrawerStatusMessage[];
  submitting?: boolean;
  submitText?: string;
  cancelText?: string;
  width?: number | string;
  onSubmit: (
    values: SchemaFormValues,
    context: SchemaFormDrawerContext,
  ) => Promise<unknown> | unknown;
  onBeforeSubmit?: (
    values: SchemaFormValues,
    context: SchemaFormDrawerContext,
  ) => Promise<boolean | void> | boolean | void;
  onSubmitSuccess?: (result: unknown, context: SchemaFormDrawerContext) => void;
  onSubmitError?: (error: unknown, context: SchemaFormDrawerContext) => void;
  onCancel?: (context: SchemaFormDrawerContext) => void;
  onValuesChange?: (
    values: SchemaFormValues,
    changed: SchemaFormValues,
    context: SchemaFormDrawerContext,
  ) => void;
  extraActions?: SchemaFormDrawerAction[];
}

const DEFAULT_SCHEMA_FORM_DRAWER_WIDTH = 560;
const DEFAULT_SCHEMA_FORM_DRAWER_MIN_WIDTH = 360;
const DEFAULT_SCHEMA_FORM_DRAWER_MAX_WIDTH = 960;

function defaultValuesFromSchema(schema: PluginFormSchema): SchemaFormValues {
  return Object.fromEntries(
    schema.fields
      .filter((field) => field.default_value !== undefined)
      .map((field) => [field.key, field.default_value])
  );
}

function fieldRules(field: PluginFormFieldSchema): Rule[] {
  const rules: Rule[] = [];
  if (field.required) {
    rules.push({
      required: true,
      message: i18nText('schemaUi', 'auto.field_required', {
        value1: field.label
      })
    });
  }
  if (field.pattern) {
    rules.push({
      pattern: new RegExp(field.pattern),
      message: i18nText('schemaUi', 'auto.field_pattern_invalid', {
        value1: field.label
      })
    });
  }
  return rules;
}

function renderFieldControl(field: PluginFormFieldSchema) {
  if (field.type === 'number') {
    return (
      <InputNumber
        className="schema-form-drawer__number"
        max={field.max}
        min={field.min}
        precision={field.precision}
        step={field.step}
      />
    );
  }

  if (field.type === 'boolean') {
    return <Switch />;
  }

  if (field.type === 'select') {
    return (
      <Select
        options={(field.options ?? []).map((option) => ({
          disabled: option.disabled,
          label: option.label,
          value: option.value as string | number | boolean
        }))}
      />
    );
  }

  if (field.control === 'textarea') {
    return <Input.TextArea autoSize={{ minRows: 3, maxRows: 8 }} placeholder={field.placeholder} />;
  }

  return <Input placeholder={field.placeholder} />;
}

function drawerTitle(title: string, subtitle?: string) {
  if (!subtitle) {
    return title;
  }

  return (
    <Space className="schema-form-drawer__title" orientation="vertical" size={0}>
      <span>{title}</span>
      <span className="schema-form-drawer__subtitle">{subtitle}</span>
    </Space>
  );
}

function errorMessageFrom(error: unknown) {
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }
  if (typeof error === 'string' && error.trim().length > 0) {
    return error;
  }
  return i18nText('schemaUi', 'auto.submit_failed');
}

export function SchemaFormDrawer({
  bodyClassName,
  cancelText,
  defaultWidth,
  disabled = false,
  extraActions = [],
  initialValues,
  leadingContent,
  maxWidth = DEFAULT_SCHEMA_FORM_DRAWER_MAX_WIDTH,
  minWidth = DEFAULT_SCHEMA_FORM_DRAWER_MIN_WIDTH,
  onBeforeSubmit,
  onCancel,
  onSubmit,
  onSubmitError,
  onSubmitSuccess,
  onValuesChange,
  open,
  resizable = false,
  resizeLabel = '调整抽屉宽度',
  rootClassName,
  schema,
  statusMessages = [],
  submitText,
  submitting,
  subtitle,
  title,
  width = DEFAULT_SCHEMA_FORM_DRAWER_WIDTH
}: SchemaFormDrawerProps) {
  const [form] = Form.useForm<SchemaFormValues>();
  const [localSubmitting, setLocalSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const contextRef = useRef<SchemaFormDrawerContext | null>(null);
  const initialResizableWidth =
    defaultWidth ??
    (typeof width === 'number' ? width : DEFAULT_SCHEMA_FORM_DRAWER_WIDTH);
  const isSubmitting = submitting ?? localSubmitting;
  const resolvedInitialValues = useMemo(
    () => ({
      ...defaultValuesFromSchema(schema),
      ...(initialValues ?? {})
    }),
    [initialValues, schema]
  );

  const getValues = useCallback(
    () => form.getFieldsValue(true) as SchemaFormValues,
    [form]
  );

  const validate = useCallback(async () => {
    try {
      await form.validateFields();
      return true;
    } catch {
      return false;
    }
  }, [form]);

  const currentContext = useCallback(() => {
    if (!contextRef.current) {
      throw new Error('SchemaFormDrawer context is not ready');
    }
    return contextRef.current;
  }, []);

  const closeWithoutSubmit = useCallback(() => {
    onCancel?.(currentContext());
  }, [currentContext, onCancel]);

  const confirmClose = useCallback(() => {
    if (!form.isFieldsTouched()) {
      closeWithoutSubmit();
      return;
    }

    Modal.confirm({
      title: i18nText('schemaUi', 'auto.discard_unsaved_changes_title'),
      content: i18nText('schemaUi', 'auto.discard_unsaved_changes_content'),
      okText: i18nText('schemaUi', 'auto.discard_changes'),
      cancelText: i18nText('schemaUi', 'auto.keep_editing'),
      onOk: closeWithoutSubmit
    });
  }, [closeWithoutSubmit, form]);

  const submitForm = useCallback(async () => {
    setSubmitError(null);
    let values: SchemaFormValues;
    try {
      values = (await form.validateFields()) as SchemaFormValues;
    } catch {
      return;
    }
    const context = currentContext();
    const shouldContinue = await onBeforeSubmit?.(values, context);
    if (shouldContinue === false) {
      return;
    }

    setLocalSubmitting(true);
    try {
      const result = await onSubmit(values, context);
      onSubmitSuccess?.(result, context);
    } catch (error) {
      setSubmitError(errorMessageFrom(error));
      onSubmitError?.(error, context);
    } finally {
      setLocalSubmitting(false);
    }
  }, [currentContext, form, onBeforeSubmit, onSubmit, onSubmitError, onSubmitSuccess]);

  const context = useMemo<SchemaFormDrawerContext>(
    () => ({
      close: confirmClose,
      getValues,
      reset: () => {
        form.resetFields();
        setSubmitError(null);
      },
      setFieldValue: (name, value) => form.setFieldValue(name, value),
      setValues: (values) => form.setFieldsValue(values),
      submit: submitForm,
      validate
    }),
    [confirmClose, form, getValues, submitForm, validate]
  );
  contextRef.current = context;

  useEffect(() => {
    if (!open) {
      return;
    }
    form.setFieldsValue(resolvedInitialValues);
    setSubmitError(null);
  }, [form, open, resolvedInitialValues]);

  const leftActions = extraActions.filter((action) => action.placement === 'left');
  const rightActions = extraActions.filter((action) => action.placement !== 'left');

  const footer = (
    <Flex
      className="schema-form-drawer__footer"
      gap="small"
      justify={leftActions.length > 0 ? 'space-between' : 'start'}
      wrap
    >
      <Space wrap>
        {leftActions.map((action) => (
          <Button
            danger={action.variant === 'danger'}
            disabled={disabled || action.disabled || isSubmitting}
            icon={action.icon}
            key={action.key}
            loading={action.loading}
            onClick={() => void action.onClick(context)}
          >
            {action.label}
          </Button>
        ))}
      </Space>
      <Space wrap>
        {rightActions.map((action) => (
          <Button
            danger={action.variant === 'danger'}
            disabled={disabled || action.disabled || isSubmitting}
            icon={action.icon}
            key={action.key}
            loading={action.loading}
            onClick={() => void action.onClick(context)}
          >
            {action.label}
          </Button>
        ))}
        <Button
          disabled={disabled}
          loading={isSubmitting}
          type="primary"
          onClick={() => void submitForm()}
        >
          {submitText ?? i18nText('schemaUi', 'auto.save')}
        </Button>
        <Button disabled={isSubmitting} onClick={confirmClose}>
          {cancelText ?? i18nText('schemaUi', 'auto.cancel')}
        </Button>
      </Space>
    </Flex>
  );
  const body = (
    <div
      className={['schema-form-drawer__body', bodyClassName]
        .filter(Boolean)
        .join(' ')}
    >
      {leadingContent}
      {statusMessages.map((statusMessage) => (
        <Alert
          key={statusMessage.key}
          title={statusMessage.message}
          showIcon
          type={statusMessage.type}
        />
      ))}
      {submitError ? <Alert title={submitError} showIcon type="error" /> : null}
      <Form<SchemaFormValues>
        disabled={disabled}
        form={form}
        initialValues={resolvedInitialValues}
        layout="vertical"
        onValuesChange={(changed, values) =>
          onValuesChange?.(
            values as SchemaFormValues,
            changed as SchemaFormValues,
            context
          )
        }
      >
        {schema.fields.map((field) => (
          <Form.Item
            extra={field.description}
            key={field.key}
            label={field.label}
            name={field.key}
            rules={fieldRules(field)}
            valuePropName={field.type === 'boolean' ? 'checked' : 'value'}
          >
            {field.read_only ? (
              <Input disabled placeholder={field.placeholder} />
            ) : (
              renderFieldControl(field)
            )}
          </Form.Item>
        ))}
      </Form>
    </div>
  );

  if (resizable) {
    return (
      <ResizableDrawer
        destroyOnClose
        defaultWidth={initialResizableWidth}
        footer={footer}
        maxWidth={maxWidth}
        minWidth={minWidth}
        open={open}
        resizeLabel={resizeLabel}
        rootClassName={['schema-form-drawer', rootClassName]
          .filter(Boolean)
          .join(' ')}
        title={drawerTitle(title, subtitle)}
        onClose={confirmClose}
      >
        {body}
      </ResizableDrawer>
    );
  }

  return (
    <Drawer
      className="schema-form-drawer"
      destroyOnHidden
      footer={footer}
      open={open}
      placement="right"
      rootClassName={rootClassName}
      title={drawerTitle(title, subtitle)}
      size={width}
      onClose={confirmClose}
    >
      {body}
    </Drawer>
  );
}
