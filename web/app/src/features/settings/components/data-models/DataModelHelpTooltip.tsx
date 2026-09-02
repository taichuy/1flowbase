import { useId } from 'react';

import QuestionCircleOutlined from '@ant-design/icons/es/icons/QuestionCircleOutlined';
import { Tooltip } from 'antd';
import { i18nText } from '../../../../shared/i18n/text';

export function DataModelFieldLabel({
  label,
  title,
  decorativeHelp = false
}: {
  label: string;
  title: string;
  decorativeHelp?: boolean;
}) {
  return (
    <span className="data-model-panel__field-label">
      <span>{label}</span>
      <DataModelHelpTooltip
        decorative={decorativeHelp}
        label={label}
        title={title}
      />
    </span>
  );
}

export function DataModelHelpTooltip({
  decorative = false,
  label,
  title
}: {
  decorative?: boolean;
  label: string;
  title: string;
}) {
  const descriptionId = useId();

  return (
    <>
      <Tooltip title={title}>
        <QuestionCircleOutlined
          aria-describedby={decorative ? undefined : descriptionId}
          aria-hidden={decorative ? true : undefined}
          aria-label={
            decorative
              ? undefined
              : i18nText('settings', 'auto.description_option', {
                  value1: label
                })
          }
          className="data-model-panel__help-icon"
          tabIndex={decorative ? -1 : 0}
        />
      </Tooltip>
      {decorative ? null : (
        <span id={descriptionId} className="data-model-panel__sr-only">
          {title}
        </span>
      )}
    </>
  );
}
