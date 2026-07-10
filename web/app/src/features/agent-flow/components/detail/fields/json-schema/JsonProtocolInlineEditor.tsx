import { Tabs, Typography } from 'antd';
import { useState, type ReactNode } from 'react';

import { i18nText } from '../../../../../../shared/i18n/text';
import { InlineJsonCodeEditor } from './JsonSchemaSettingsPanel';
import type { SchemaEditorTab } from './schema-row-model';

export type JsonProtocolEditorResult<TValue> =
  | { ok: true; value: TValue }
  | { ok: false; message: string };

export function JsonProtocolInlineEditor<TValue>({
  ariaLabel,
  className,
  testId,
  hint,
  value,
  stringifyValue,
  parseValue,
  renderFields,
  onChange,
  onValidityChange
}: {
  ariaLabel: string;
  className?: string;
  testId?: string;
  hint?: ReactNode;
  value: TValue;
  stringifyValue: (value: TValue) => string;
  parseValue: (value: string) => JsonProtocolEditorResult<TValue>;
  renderFields: (params: {
    value: TValue;
    onChange: (nextValue: TValue) => void;
  }) => ReactNode;
  onChange: (value: TValue) => void;
  onValidityChange?: (valid: boolean) => void;
}) {
  const [activeTab, setActiveTab] = useState<SchemaEditorTab>('fields');
  const [jsonText, setJsonText] = useState(() => stringifyValue(value));
  const [protocolError, setProtocolError] = useState<string | null>(null);

  function setValidValue(nextValue: TValue) {
    setProtocolError(null);
    onValidityChange?.(true);
    onChange(nextValue);
  }

  function switchTab(nextTab: string) {
    if (nextTab === 'json') {
      setJsonText(stringifyValue(value));
      setProtocolError(null);
      onValidityChange?.(true);
      setActiveTab('json');
      return;
    }

    const parsed = parseValue(jsonText);

    if (!parsed.ok) {
      setProtocolError(parsed.message);
      onValidityChange?.(false);
      return;
    }

    setValidValue(parsed.value);
    setActiveTab('fields');
  }

  function updateJsonText(nextValue: string) {
    setJsonText(nextValue);
    const parsed = parseValue(nextValue);

    if (!parsed.ok) {
      setProtocolError(parsed.message);
      onValidityChange?.(false);
      return;
    }

    setValidValue(parsed.value);
  }

  return (
    <div
      className={['agent-flow-json-schema-settings', className]
        .filter(Boolean)
        .join(' ')}
      data-testid={testId}
    >
      <Tabs
        activeKey={activeTab}
        items={[
          {
            key: 'fields',
            label: i18nText('agentFlow', 'auto.schema_fields'),
            children: renderFields({
              value,
              onChange: setValidValue
            })
          },
          {
            key: 'json',
            label: i18nText('agentFlow', 'auto.json_parse'),
            children: (
              <InlineJsonCodeEditor
                ariaLabel={ariaLabel}
                value={jsonText}
                onChange={updateJsonText}
              />
            )
          }
        ]}
        onChange={switchTab}
      />
      <Typography.Text type={protocolError ? 'danger' : 'secondary'}>
        {protocolError ??
          hint ??
          i18nText('agentFlow', 'auto.json_schema_parse_hint')}
      </Typography.Text>
    </div>
  );
}
