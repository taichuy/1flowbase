import { Cascader, Select } from 'antd';

import {
  decodeSelectorValue,
  encodeSelectorValue,
  type FlowSelectorOption,
  toCascaderSelectorOptions
} from '../../lib/selector-options';
import { i18nText } from '../../../../shared/i18n/text';

interface SelectorFieldProps {
  ariaLabel: string;
  value: string[] | string[][];
  options: FlowSelectorOption[];
  multiple?: boolean;
  onChange: (value: string[] | string[][]) => void;
}

function isSelectorListValue(
  value: string[] | string[][]
): value is string[][] {
  return value.length === 0 || Array.isArray(value[0]);
}

export function SelectorField({
  ariaLabel,
  value,
  options,
  multiple = false,
  onChange
}: SelectorFieldProps) {
  if (multiple) {
    const selectedValues = isSelectorListValue(value)
      ? value.map((item) => encodeSelectorValue(item))
      : [];

    return (
      <Select
        aria-label={ariaLabel}
        mode="multiple"
        options={options.map((option) => ({
          label: option.displayLabel,
          value: encodeSelectorValue(option.value)
        }))}
        placeholder={i18nText('agentFlow', 'auto.select_upstream_output')}
        value={selectedValues}
        onChange={(nextValues) =>
          onChange(
            nextValues.map((nextValue) =>
              decodeSelectorValue(String(nextValue))
            )
          )
        }
      />
    );
  }

  return (
    <Cascader
      changeOnSelect
      allowClear
      aria-label={ariaLabel}
      options={toCascaderSelectorOptions(options)}
      placeholder={i18nText('agentFlow', 'auto.select_upstream_output')}
      value={isSelectorListValue(value) ? [] : value}
      onChange={(nextValue) => {
        const selector = Array.isArray(nextValue) ? nextValue.map(String) : [];
        if (
          selector.length === 0 ||
          options.some(
            (option) =>
              option.value.length === selector.length &&
              option.value.every(
                (segment, index) => segment === selector[index]
              )
          )
        )
          onChange(selector);
      }}
    />
  );
}
