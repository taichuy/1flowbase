import { defaultLoopVariableValue } from '../../../../lib/variables/container-variables';
import { Input, InputNumber, Select, Switch } from 'antd';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { NamedBindingExpression } from '@1flowbase/flow-schema';
import type { FlowSelectorOption } from '../../../../lib/selector-options';
import { SelectorField } from '../../../bindings/SelectorField';
import { TemplatedTextField } from '../../../bindings/TemplatedTextField';

function StructuredConstant({
  value,
  type,
  label,
  onChange
}: {
  value: unknown;
  type: string;
  label: string;
  onChange: (value: unknown) => void;
}) {
  const [draft, setDraft] = useState(() => JSON.stringify(value));
  const [invalid, setInvalid] = useState(false);
  useEffect(() => {
    setDraft(JSON.stringify(value));
    setInvalid(false);
  }, [value]);
  return (
    <Input.TextArea
      aria-label={label}
      value={draft}
      status={invalid ? 'error' : undefined}
      autoSize={{ minRows: 2, maxRows: 6 }}
      onChange={(event) => {
        setDraft(event.target.value);
        try {
          const next: unknown = JSON.parse(event.target.value);
          const valid =
            type === 'array'
              ? Array.isArray(next)
              : typeof next === 'object' &&
                next !== null &&
                !Array.isArray(next);
          setInvalid(!valid);
          if (valid) onChange(next);
        } catch {
          setInvalid(true);
        }
      }}
    />
  );
}
export function ContainerValueField({
  label,
  valueType,
  expression,
  options,
  onChange,
  required = false
}: {
  required?: boolean;
  label: string;
  valueType: string;
  expression?: NamedBindingExpression | null;
  options: FlowSelectorOption[];
  onChange: (value: NamedBindingExpression | null) => void;
}) {
  const { t } = useTranslation('agentFlow');
  const mode = expression?.kind ?? (required ? 'constant' : 'keep');
  const value =
    expression?.kind === 'constant'
      ? expression.value
      : defaultLoopVariableValue(valueType);
  const matchingOptions = options.filter(
    (option) =>
      option.valueType === valueType ||
      option.valueType === 'json' ||
      (valueType === 'array' && option.valueType.startsWith('array'))
  );
  const constant = (value: unknown) => onChange({ kind: 'constant', value });
  return (
    <div className="agent-flow-container-field__value">
      <Select
        aria-label={`${label} ${t('containers.value_source')}`}
        value={mode}
        options={[
          ...(!required
            ? [{ value: 'keep', label: t('containers.keep_value') }]
            : []),
          { value: 'constant', label: t('auto.constant') },
          { value: 'selector', label: t('containers.variable_source') },
          ...(['string', 'number'].includes(valueType)
            ? [{ value: 'templated_text', label: t('containers.expression') }]
            : [])
        ]}
        onChange={(mode) =>
          onChange(
            mode === 'keep'
              ? null
              : mode === 'selector'
                ? { kind: 'selector', selector: [] }
                : mode === 'templated_text'
                  ? { kind: 'templated_text', value: '' }
                  : {
                      kind: 'constant',
                      value: defaultLoopVariableValue(valueType)
                    }
          )
        }
      />
      {mode === 'selector' ? (
        <SelectorField
          ariaLabel={label}
          options={matchingOptions}
          value={expression?.kind === 'selector' ? expression.selector : []}
          onChange={(selector) =>
            onChange({ kind: 'selector', selector: selector as string[] })
          }
        />
      ) : mode === 'templated_text' ? (
        <TemplatedTextField
          label={label}
          ariaLabel={label}
          options={options}
          value={expression?.kind === 'templated_text' ? expression.value : ''}
          onChange={(value) => onChange({ kind: 'templated_text', value })}
        />
      ) : mode === 'constant' ? (
        valueType === 'number' ? (
          <InputNumber
            aria-label={label}
            value={typeof value === 'number' ? value : null}
            onChange={constant}
          />
        ) : valueType === 'boolean' ? (
          <Switch
            aria-label={label}
            checked={value === true}
            onChange={constant}
          />
        ) : ['array', 'object'].includes(valueType) ? (
          <StructuredConstant
            label={label}
            type={valueType}
            value={value}
            onChange={constant}
          />
        ) : (
          <Input
            aria-label={label}
            value={typeof value === 'string' ? value : ''}
            onChange={(event) => constant(event.target.value)}
          />
        )
      ) : null}
    </div>
  );
}
