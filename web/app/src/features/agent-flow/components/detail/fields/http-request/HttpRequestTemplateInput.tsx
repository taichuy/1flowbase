import type { ReactNode } from 'react';

import type { FlowSelectorOption } from '../../../../lib/selector-options';
import { TemplatedTextField } from '../../../bindings/TemplatedTextField';
import { i18nText } from '../../../../../../shared/i18n/text';

interface HttpRequestTemplateInputProps {
  ariaLabel: string;
  label: string;
  labelContent?: ReactNode;
  displayMode?: 'block' | 'input';
  placeholder?: string;
  options: FlowSelectorOption[];
  value: string;
  onChange: (value: string) => void;
}

export function HttpRequestTemplateInput({
  ariaLabel,
  label,
  labelContent,
  displayMode = 'input',
  placeholder = i18nText(
    'agentFlow',
    'auto.support_text_variable_block_enter_left_curly_bracket_quick_reference'
  ),
  options,
  value,
  onChange
}: HttpRequestTemplateInputProps) {
  return (
    <TemplatedTextField
      ariaLabel={ariaLabel}
      displayMode={displayMode}
      label={label}
      labelContent={labelContent}
      options={options}
      placeholder={placeholder}
      value={value}
      onChange={onChange}
    />
  );
}
