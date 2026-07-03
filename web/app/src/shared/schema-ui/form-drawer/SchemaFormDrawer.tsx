import { Alert, Button, Drawer, Flex, Form, Input, InputNumber, Modal, Select, Space, Switch } from 'antd';
import type { Rule } from 'antd/es/form';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import { i18nText } from '../../i18n/text';
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
let schemaFormDrawerInstanceSeed = 0;

function clampSchemaFormDrawerWidth(
  width: number,
  minWidth: number,
  maxWidth: number
) {
  return Math.min(maxWidth, Math.max(minWidth, width));
}

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
    <Space className="schema-form-drawer__title" direction="vertical" size={0}>
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
  const instanceClassNameRef = useRef<string | null>(null);
  if (instanceClassNameRef.current == null) {
    schemaFormDrawerInstanceSeed += 1;
    instanceClassNameRef.current = `schema-form-drawer-instance-${schemaFormDrawerInstanceSeed}`;
  }
  const initialResizableWidth =
    defaultWidth ??
    (typeof width === 'number' ? width : DEFAULT_SCHEMA_FORM_DRAWER_WIDTH);
  const [resizableWidth, setResizableWidth] = useState(() =>
    clampSchemaFormDrawerWidth(initialResizableWidth, minWidth, maxWidth)
  );
  const dragStartRef = useRef<{ pointerX: number; width: number } | null>(null);
  const resizeFrameRef = useRef<number | null>(null);
  const pendingDrawerWidthRef = useRef<number | null>(null);
  const liveDrawerWidthRef = useRef(resizableWidth);
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

  useEffect(() => {
    if (!open || !resizable) {
      return;
    }
    const nextWidth = clampSchemaFormDrawerWidth(
      initialResizableWidth,
      minWidth,
      maxWidth
    );
    liveDrawerWidthRef.current = nextWidth;
    setResizableWidth(nextWidth);
  }, [initialResizableWidth, maxWidth, minWidth, open, resizable]);

  useEffect(() => {
    liveDrawerWidthRef.current = resizableWidth;
  }, [resizableWidth]);

  useEffect(() => {
    if (!resizable) {
      return undefined;
    }

    const drawerRootSelector = `.${instanceClassNameRef.current}`;
    const applyLiveDrawerWidth = (nextWidth: number) => {
      liveDrawerWidthRef.current = nextWidth;
      const drawerWrapper = document.querySelector<HTMLElement>(
        `${drawerRootSelector} .ant-drawer-content-wrapper`
      );
      if (drawerWrapper) {
        drawerWrapper.style.width = `${nextWidth}px`;
      }
    };

    const handleMouseMove = (event: MouseEvent) => {
      const dragStart = dragStartRef.current;
      if (!dragStart) {
        return;
      }

      pendingDrawerWidthRef.current = clampSchemaFormDrawerWidth(
        dragStart.width + dragStart.pointerX - event.clientX,
        minWidth,
        maxWidth
      );
      if (resizeFrameRef.current != null) {
        return;
      }

      resizeFrameRef.current = window.requestAnimationFrame(() => {
        resizeFrameRef.current = null;
        const nextWidth = pendingDrawerWidthRef.current;
        pendingDrawerWidthRef.current = null;
        if (nextWidth == null) {
          return;
        }
        applyLiveDrawerWidth(nextWidth);
      });
    };

    const handleMouseUp = () => {
      const pendingWidth = pendingDrawerWidthRef.current;
      if (resizeFrameRef.current != null) {
        window.cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
      if (pendingWidth != null) {
        pendingDrawerWidthRef.current = null;
        applyLiveDrawerWidth(pendingWidth);
      }
      setResizableWidth((currentWidth) =>
        currentWidth === liveDrawerWidthRef.current
          ? currentWidth
          : liveDrawerWidthRef.current
      );
      dragStartRef.current = null;
      document.body.classList.remove('schema-form-drawer--resizing');
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      if (resizeFrameRef.current != null) {
        window.cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
      document.body.classList.remove('schema-form-drawer--resizing');
    };
  }, [maxWidth, minWidth, resizable]);

  const leftActions = extraActions.filter((action) => action.placement === 'left');
  const rightActions = extraActions.filter((action) => action.placement !== 'left');
  const resolvedRootClassName = [
    rootClassName,
    resizable ? instanceClassNameRef.current : null
  ]
    .filter(Boolean)
    .join(' ');
  const resolvedWidth = resizable ? resizableWidth : width;

  return (
    <Drawer
      className="schema-form-drawer"
      destroyOnClose
      footer={
        <Flex className="schema-form-drawer__footer" gap="small" justify={leftActions.length > 0 ? 'space-between' : 'start'} wrap>
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
            <Button disabled={disabled} loading={isSubmitting} type="primary" onClick={() => void submitForm()}>
              {submitText ?? i18nText('schemaUi', 'auto.save')}
            </Button>
            <Button disabled={isSubmitting} onClick={confirmClose}>
              {cancelText ?? i18nText('schemaUi', 'auto.cancel')}
            </Button>
          </Space>
        </Flex>
      }
	      open={open}
	      placement="right"
	      rootClassName={resolvedRootClassName}
      title={drawerTitle(title, subtitle)}
      width={resolvedWidth}
      onClose={confirmClose}
    >
	      <div className={['schema-form-drawer__body', bodyClassName].filter(Boolean).join(' ')}>
	        {resizable ? (
	          <div
	            aria-label={resizeLabel}
	            aria-orientation="vertical"
	            aria-valuemax={maxWidth}
	            aria-valuemin={minWidth}
	            aria-valuenow={resizableWidth}
	            className="schema-form-drawer__resize-handle"
	            role="separator"
	            tabIndex={0}
	            onKeyDown={(event) => {
	              if (event.key === 'ArrowLeft') {
	                event.preventDefault();
	                setResizableWidth((currentWidth) =>
	                  clampSchemaFormDrawerWidth(
	                    currentWidth + 40,
	                    minWidth,
	                    maxWidth
	                  )
	                );
	                return;
	              }

	              if (event.key === 'ArrowRight') {
	                event.preventDefault();
	                setResizableWidth((currentWidth) =>
	                  clampSchemaFormDrawerWidth(
	                    currentWidth - 40,
	                    minWidth,
	                    maxWidth
	                  )
	                );
	                return;
	              }

	              if (event.key === 'Home') {
	                event.preventDefault();
	                setResizableWidth(minWidth);
	                return;
	              }

	              if (event.key === 'End') {
	                event.preventDefault();
	                setResizableWidth(maxWidth);
	              }
	            }}
	            onMouseDown={(event) => {
	              event.preventDefault();
	              dragStartRef.current = {
	                pointerX: event.clientX,
	                width: liveDrawerWidthRef.current
	              };
	              document.body.classList.add('schema-form-drawer--resizing');
	            }}
	          />
	        ) : null}
	        {leadingContent}
	        {statusMessages.map((statusMessage) => (
	          <Alert
	            key={statusMessage.key}
	            message={statusMessage.message}
	            showIcon
	            type={statusMessage.type}
	          />
	        ))}
	        {submitError ? <Alert message={submitError} showIcon type="error" /> : null}
	        <Form<SchemaFormValues>
	          disabled={disabled}
          form={form}
          initialValues={resolvedInitialValues}
          layout="vertical"
          onValuesChange={(changed, values) =>
            onValuesChange?.(values as SchemaFormValues, changed as SchemaFormValues, context)
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
    </Drawer>
  );
}
