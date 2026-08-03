import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Input, InputNumber, Select, Switch, Typography } from 'antd';
import type { RefObject } from 'react';

import type { ConsoleApplicationNodeContractField } from '@1flowbase/api-client';

import type {
  FlowStartInputField,
  FlowStartInputType
} from '@1flowbase/flow-schema';

import {
  getStartInputValueType,
  startInputTypeOptions
} from '../../../lib/variables/start-node-variables';
import { FloatingSettingsPanel } from '../FloatingSettingsPanel';
import { i18nText } from '../../../../../shared/i18n/text';
import type { StartInputSourceOption } from './StartInputFieldsField';

type StartInputFieldSettingsPanelProps = {
  mode: 'create' | 'edit';
  field: FlowStartInputField;
  inputTypeContract?: ConsoleApplicationNodeContractField;
  inputTypeOptions: typeof startInputTypeOptions;
  sourceOptions: StartInputSourceOption[];
  sourceContract?: ConsoleApplicationNodeContractField;
  triggerRef: RefObject<HTMLElement | null>;
  onChange: (patch: Partial<FlowStartInputField>) => void;
  onClose: () => void;
  onSave: () => void;
};

function isStringDefaultType(inputType: FlowStartInputType) {
  return (
    inputType === 'text' ||
    inputType === 'paragraph' ||
    inputType === 'select' ||
    inputType === 'url'
  );
}

function shouldShowMaxLength(inputType: FlowStartInputType) {
  return (
    inputType === 'text' || inputType === 'paragraph' || inputType === 'url'
  );
}

function normalizeOptions(options: string[] | undefined) {
  const nextOptions = options?.length ? options : [''];

  return nextOptions;
}

export function StartInputFieldSettingsPanel({
  mode,
  field,
  inputTypeContract,
  inputTypeOptions,
  sourceOptions,
  sourceContract,
  triggerRef,
  onChange,
  onClose,
  onSave
}: StartInputFieldSettingsPanelProps) {
  const title =
    mode === 'create'
      ? i18nText('agentFlow', 'auto.add_new_input_field')
      : i18nText('agentFlow', 'auto.edit_input_field_alt');
  const options = normalizeOptions(field.options);
  const showDefaultValue =
    isStringDefaultType(field.inputType) ||
    field.inputType === 'number' ||
    field.inputType === 'checkbox';

  function handleTypeChange(inputType: FlowStartInputType) {
    const valueType = getStartInputValueType(inputType);

    onChange({
      inputType,
      valueType,
      options: inputType === 'select' ? options : undefined,
      defaultValue: undefined,
      maxLength: shouldShowMaxLength(inputType) ? field.maxLength : undefined
    });
  }

  function updateOption(index: number, value: string) {
    onChange({
      options: options.map((option, optionIndex) =>
        optionIndex === index ? value : option
      )
    });
  }

  function removeOption(index: number) {
    const nextOptions = options.filter(
      (_, optionIndex) => optionIndex !== index
    );

    onChange({
      options: nextOptions.length > 0 ? nextOptions : [''],
      defaultValue: nextOptions.includes(String(field.defaultValue ?? ''))
        ? field.defaultValue
        : undefined
    });
  }

  return (
    <FloatingSettingsPanel
      open
      title={title}
      closeLabel={i18nText('agentFlow', 'auto.close', { value1: title })}
      triggerRef={triggerRef}
      className="agent-flow-start-input-fields__panel"
      defaultWidth={420}
      minWidth={360}
      initialHeight={520}
      gap={16}
      onClose={onClose}
      footer={
        <div className="agent-flow-start-input-fields__panel-footer">
          <Button onClick={onClose}>
            {i18nText('agentFlow', 'auto.cancel')}
          </Button>
          <Button
            aria-label={i18nText('agentFlow', 'auto.save_input_field')}
            type="primary"
            onClick={onSave}
          >
            {i18nText('agentFlow', 'auto.save')}
          </Button>
        </div>
      }
    >
      <div className="agent-flow-start-input-fields__form">
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.field_type')}</span>
          <Select
            aria-label={i18nText('agentFlow', 'auto.input_field_type')}
            aria-required={inputTypeContract?.required}
            options={inputTypeOptions}
            value={field.inputType}
            virtual={false}
            onChange={handleTypeChange}
          />
          {inputTypeContract?.description ? (
            <Typography.Text type="secondary">
              {inputTypeContract.description}
            </Typography.Text>
          ) : null}
        </div>
        {sourceOptions.length > 0 ? (
          <div className="agent-flow-start-input-fields__form-row">
            <span>{i18nText('agentFlow', 'auto.parameter_source')}</span>
            <Select
              aria-label={i18nText(
                'agentFlow',
                'auto.input_field_parameter_source'
              )}
              aria-required={sourceContract?.required}
              options={sourceOptions}
              value={field.source}
              virtual={false}
              onChange={(source) => onChange({ source })}
            />
            {sourceContract?.description ? (
              <Typography.Text type="secondary">
                {sourceContract.description}
                {sourceContract.applicability
                  ? ` ${sourceContract.applicability}`
                  : ''}
              </Typography.Text>
            ) : null}
          </div>
        ) : null}
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.variable_name')}</span>
          <Input
            aria-label={i18nText('agentFlow', 'auto.input_field_variable_name')}
            value={field.key}
            onChange={(event) => onChange({ key: event.target.value })}
          />
        </div>
        <div className="agent-flow-start-input-fields__form-row">
          <span>{i18nText('agentFlow', 'auto.display_name')}</span>
          <Input
            aria-label={i18nText('agentFlow', 'auto.input_field_display_name')}
            value={field.label}
            onChange={(event) => onChange({ label: event.target.value })}
          />
        </div>
        {shouldShowMaxLength(field.inputType) ? (
          <div className="agent-flow-start-input-fields__form-row">
            <span>{i18nText('agentFlow', 'auto.maximum_length')}</span>
            <InputNumber
              aria-label={i18nText(
                'agentFlow',
                'auto.maximum_input_field_length'
              )}
              min={1}
              precision={0}
              value={field.maxLength}
              onChange={(maxLength) =>
                onChange({
                  maxLength:
                    typeof maxLength === 'number' ? maxLength : undefined
                })
              }
            />
          </div>
        ) : null}

        {field.inputType === 'select' ? (
          <div className="agent-flow-start-input-fields__form-row">
            <span>{i18nText('agentFlow', 'auto.drop_down_options')}</span>
            <div className="agent-flow-start-input-fields__option-list">
              {options.map((option, index) => (
                <div
                  className="agent-flow-start-input-fields__option-row"
                  key={index}
                >
                  <Input
                    aria-label={i18nText(
                      'agentFlow',
                      'auto.input_field_options',
                      { value1: index + 1 }
                    )}
                    value={option}
                    onChange={(event) =>
                      updateOption(index, event.target.value)
                    }
                  />
                  <Button
                    aria-label={i18nText(
                      'agentFlow',
                      'auto.remove_dropdown_option',
                      { value1: index + 1 }
                    )}
                    icon={<DeleteOutlined />}
                    size="small"
                    type="text"
                    onClick={() => removeOption(index)}
                  />
                </div>
              ))}
              <Button
                aria-label={i18nText(
                  'agentFlow',
                  'auto.added_drop_down_options'
                )}
                icon={<PlusOutlined />}
                size="small"
                onClick={() => onChange({ options: [...options, ''] })}
              >
                {i18nText('agentFlow', 'auto.new_options')}
              </Button>
            </div>
          </div>
        ) : null}

        {showDefaultValue ? (
          <div className="agent-flow-start-input-fields__form-row">
            <span>{i18nText('agentFlow', 'auto.default_value')}</span>
            {field.inputType === 'paragraph' ? (
              <Input.TextArea
                aria-label={i18nText('agentFlow', 'auto.input_field_value')}
                autoSize={{ minRows: 2, maxRows: 4 }}
                value={String(field.defaultValue ?? '')}
                onChange={(event) =>
                  onChange({ defaultValue: event.target.value || undefined })
                }
              />
            ) : field.inputType === 'number' ? (
              <InputNumber
                aria-label={i18nText('agentFlow', 'auto.input_field_value')}
                value={
                  typeof field.defaultValue === 'number'
                    ? field.defaultValue
                    : undefined
                }
                onChange={(defaultValue) =>
                  onChange({
                    defaultValue:
                      typeof defaultValue === 'number'
                        ? defaultValue
                        : undefined
                  })
                }
              />
            ) : field.inputType === 'checkbox' ? (
              <Select
                aria-label={i18nText('agentFlow', 'auto.input_field_value')}
                options={[
                  {
                    value: true,
                    label: i18nText('agentFlow', 'auto.selected_by_default')
                  },
                  {
                    value: false,
                    label: i18nText('agentFlow', 'auto.selected')
                  }
                ]}
                value={
                  typeof field.defaultValue === 'boolean'
                    ? field.defaultValue
                    : undefined
                }
                virtual={false}
                onChange={(defaultValue: boolean) => onChange({ defaultValue })}
              />
            ) : field.inputType === 'select' ? (
              <Select
                allowClear
                aria-label={i18nText('agentFlow', 'auto.input_field_value')}
                options={options
                  .map((option) => option.trim())
                  .filter(Boolean)
                  .map((option) => ({ value: option, label: option }))}
                value={
                  typeof field.defaultValue === 'string'
                    ? field.defaultValue
                    : undefined
                }
                virtual={false}
                onChange={(defaultValue?: string) => onChange({ defaultValue })}
              />
            ) : (
              <Input
                aria-label={i18nText('agentFlow', 'auto.input_field_value')}
                value={String(field.defaultValue ?? '')}
                onChange={(event) =>
                  onChange({ defaultValue: event.target.value || undefined })
                }
              />
            )}
          </div>
        ) : null}

        <div className="agent-flow-start-input-fields__toggles">
          <label className="agent-flow-start-input-fields__toggle-row">
            <span>
              <Typography.Text strong>
                {i18nText('agentFlow', 'auto.required')}
              </Typography.Text>
              <Typography.Text type="secondary">
                {i18nText('agentFlow', 'auto.user_must_provide_input_running')}
              </Typography.Text>
            </span>
            <Switch
              aria-label={i18nText('agentFlow', 'auto.required_input_fields')}
              checked={field.required}
              onChange={(required) =>
                onChange({ required, hidden: required ? false : field.hidden })
              }
            />
          </label>
          <label className="agent-flow-start-input-fields__toggle-row">
            <span>
              <Typography.Text strong>
                {i18nText('agentFlow', 'auto.hide')}
              </Typography.Text>
              <Typography.Text type="secondary">
                {i18nText(
                  'agentFlow',
                  'auto.displayed_run_form_still_used_variable'
                )}
              </Typography.Text>
            </span>
            <Switch
              aria-label={i18nText('agentFlow', 'auto.hide_input_fields')}
              checked={field.hidden}
              disabled={field.required}
              onChange={(hidden) =>
                onChange({ hidden, required: hidden ? false : field.required })
              }
            />
          </label>
        </div>
      </div>
    </FloatingSettingsPanel>
  );
}
