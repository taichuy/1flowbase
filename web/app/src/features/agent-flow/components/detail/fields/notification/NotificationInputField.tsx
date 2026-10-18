import { cloneElement, useEffect, useState, type ReactElement } from 'react';
import { Button, Flex, Select, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import type { NamedBindingExpression } from '@1flowbase/flow-schema';
import type { PluginFormFieldSchema } from '../../../../../../shared/schema-ui/v1/contracts/plugin-form-schema';
import { renderSchemaFormFieldControl } from '../../../../../../shared/schema-ui/v1/form-drawer/schema-form-field-control';
import type { FlowSelectorOption } from '../../../../lib/selector-options';
import { SelectorField } from '../../../bindings/SelectorField';
import { TemplatedTextField } from '../../../bindings/TemplatedTextField';
import { InlineJsonCodeEditor } from '../json-schema/JsonSchemaSettingsPanel';

function defaultConstant(valueType: string) {
  if (valueType === 'array') return [];
  if (valueType === 'object') return {};
  if (valueType === 'boolean') return false;
  if (valueType === 'number') return 0;
  return '';
}
function JsonConstantField({
  label,
  value,
  valueType,
  onChange
}: {
  label: string;
  value: unknown;
  valueType: string;
  onChange: (value: unknown) => void;
}) {
  const { t } = useTranslation('agentFlow');
  const [draft, setDraft] = useState(() =>
    JSON.stringify(value ?? defaultConstant(valueType), null, 2)
  );
  const [invalid, setInvalid] = useState(false);
  useEffect(() => {
    setDraft(JSON.stringify(value ?? defaultConstant(valueType), null, 2));
    setInvalid(false);
  }, [value, valueType]);
  return (
    <>
      <InlineJsonCodeEditor
        ariaLabel={label}
        value={draft}
        onChange={(text) => {
          setDraft(text);
          try {
            const parsed: unknown = JSON.parse(text);
            const valid =
              valueType === 'array'
                ? Array.isArray(parsed)
                : typeof parsed === 'object' &&
                  parsed !== null &&
                  !Array.isArray(parsed);
            setInvalid(!valid);
            if (valid) onChange(parsed);
          } catch {
            setInvalid(true);
          }
        }}
      />
      {invalid && (
        <Typography.Text type="danger">
          {t('auto.notification_invalid_constant')}
        </Typography.Text>
      )}
    </>
  );
}
export function NotificationInputField({
  field,
  valueType,
  expression,
  options,
  onChange,
  onClear
}: {
  field: PluginFormFieldSchema;
  valueType: string;
  expression?: NamedBindingExpression | null;
  options: FlowSelectorOption[];
  onChange: (expression: NamedBindingExpression) => void;
  onClear: () => void;
}) {
  const { t } = useTranslation('agentFlow');
  const mode = expression?.kind ?? 'constant';
  const allowTemplate = valueType === 'string' || valueType === 'number';
  const literal =
    expression?.kind === 'constant' ? expression.value : field.default_value;
  const selectorOptions = options.filter(
    (option) =>
      valueType === 'string' ||
      option.valueType === valueType ||
      (valueType === 'object' && option.valueType === 'json') ||
      (valueType === 'array' && option.valueType.startsWith('array'))
  );
  const fieldWithControl = {
    ...field,
    type: field.type === 'select' ? 'select' : valueType,
    control:
      valueType === 'array' && field.options?.length
        ? 'multi-select'
        : field.control
  };
  function constantControl() {
    if (
      (valueType === 'array' &&
        !field.options?.length &&
        field.control !== 'multi-select') ||
      valueType === 'object'
    ) {
      return (
        <JsonConstantField
          label={field.label}
          value={literal}
          valueType={valueType}
          onChange={(value) => onChange({ kind: 'constant', value })}
        />
      );
    }
    const control = renderSchemaFormFieldControl(
      fieldWithControl
    ) as ReactElement<{
      value?: unknown;
      checked?: boolean;
      'aria-label'?: string;
      onChange?: (value: unknown) => void;
    }>;
    return cloneElement(control, {
      'aria-label': field.label,
      ...(fieldWithControl.type === 'boolean'
        ? { checked: literal === true }
        : { value: literal }),
      onChange: (value) => {
        const next =
          typeof value === 'object' && value !== null && 'target' in value
            ? (value as { target: { value: string } }).target.value
            : value;
        onChange({ kind: 'constant', value: next });
      }
    });
  }
  return (
    <div
      className="agent-flow-editor__inspector-field"
      data-field-key={`bindings.input.${field.key}`}
    >
      <Flex gap="small" align="center" justify="space-between" wrap>
        <Typography.Text strong>
          {field.label}
          {field.required ? ' *' : ''}
        </Typography.Text>
        <Flex gap="small" align="center">
          <Select
            aria-label={t('auto.notification_binding_mode', {
              field: field.label
            })}
            size="small"
            value={mode}
            options={[
              { value: 'constant', label: t('auto.constant') },
              {
                value: 'selector',
                label: t('auto.notification_variable_source')
              },
              ...(allowTemplate
                ? [
                    {
                      value: 'templated_text',
                      label:
                        valueType === 'number'
                          ? t('auto.notification_expression')
                          : t('auto.template')
                    }
                  ]
                : [])
            ]}
            onChange={(kind) =>
              onChange(
                kind === 'selector'
                  ? { kind, selector: [] }
                  : kind === 'templated_text'
                    ? { kind, value: '' }
                    : {
                        kind: 'constant',
                        value: field.default_value ?? defaultConstant(valueType)
                      }
              )
            }
          />
          {!field.required && (
            <Button type="text" size="small" onClick={onClear}>
              {t('auto.clear')}
            </Button>
          )}
        </Flex>
      </Flex>
      {mode === 'selector' ? (
        <SelectorField
          ariaLabel={field.label}
          options={selectorOptions}
          value={expression?.kind === 'selector' ? expression.selector : []}
          onChange={(selector) =>
            onChange({ kind: 'selector', selector: selector as string[] })
          }
        />
      ) : mode === 'templated_text' ? (
        <TemplatedTextField
          label={field.label}
          ariaLabel={field.label}
          options={selectorOptions}
          value={expression?.kind === 'templated_text' ? expression.value : ''}
          onChange={(value) => onChange({ kind: 'templated_text', value })}
        />
      ) : (
        constantControl()
      )}
      {!expression && (
        <Typography.Text type="secondary">
          {t('auto.notification_input_unset')}
        </Typography.Text>
      )}
      {field.description && (
        <Typography.Text type="secondary">{field.description}</Typography.Text>
      )}
    </div>
  );
}
