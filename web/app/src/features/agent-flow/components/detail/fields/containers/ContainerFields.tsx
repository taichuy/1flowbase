import {
  Button,
  Flex,
  Input,
  InputNumber,
  Select,
  Switch,
  Typography
} from 'antd';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import { useTranslation } from 'react-i18next';
import type {
  FlowBinding,
  FlowConditionGroupDocument,
  FlowNodeDocument,
  NamedBindingEntry,
  NamedBindingExpression
} from '@1flowbase/flow-schema';
import type { SchemaFieldRendererProps } from '../../../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import type { FlowSelectorOption } from '../../../../lib/selector-options';
import { loopVariableDefinitions } from '../../../../lib/variables/container-variables';
import { useStableListItemKeys } from '../../../../hooks/interactions/use-stable-list-item-keys';
import { SelectorField } from '../../../bindings/SelectorField';
import { ConditionGroupField } from '../../../bindings/ConditionGroupField';
import { ContainerValueField } from './ContainerValueField';
import './container-fields.css';

function optionsFor(
  adapter: SchemaFieldRendererProps['adapter'],
  path: string
) {
  return (
    (adapter.getDerived(`selectorOptions:${path}`) as
      | FlowSelectorOption[]
      | null) ?? []
  );
}
function entries(value: unknown): NamedBindingEntry[] {
  const binding = value as FlowBinding | undefined;
  return binding?.kind === 'named_bindings' ? binding.value : [];
}
export function ContainerVariablesField({ adapter }: SchemaFieldRendererProps) {
  const { t } = useTranslation('agentFlow');
  const node = adapter.getDerived('node') as FlowNodeDocument;
  const variables = loopVariableDefinitions(node);
  const rowKeys = useStableListItemKeys(
    `container-variable-${node.id}`,
    variables.length
  );
  const initial = entries(adapter.getValue('bindings.variables'));
  const updates = entries(adapter.getValue('bindings.variable_updates'));
  function updateBinding(
    path: string,
    key: string,
    expression: NamedBindingExpression | null
  ) {
    const current = entries(adapter.getValue(path)).filter(
      (entry) => entry.name !== key
    );
    adapter.setValue(path, {
      kind: 'named_bindings',
      value: expression
        ? [...current, { name: key, value: expression }]
        : current
    });
  }
  return (
    <div
      className="agent-flow-container-field"
      data-testid="container-loop-variables"
    >
      {variables.map((variable, index) => (
        <div
          className="agent-flow-container-field__row"
          key={rowKeys.itemKeys[index]}
        >
          <Flex gap={8}>
            <Input
              aria-label={`${t('containers.variable_name')} ${index + 1}`}
              value={variable.key}
              onChange={(event) =>
                adapter.setValue(
                  'config.loop_variables',
                  variables.map((entry, i) =>
                    i === index ? { ...entry, key: event.target.value } : entry
                  )
                )
              }
            />
            <Select
              aria-label={`${t('containers.variable_type')} ${index + 1}`}
              value={variable.value_type}
              options={[
                { value: 'string', label: t('auto.string') },
                { value: 'number', label: t('auto.number') },
                { value: 'boolean', label: t('auto.boolean') },
                { value: 'object', label: t('auto.object') },
                { value: 'array', label: t('auto.array') }
              ]}
              onChange={(value_type) =>
                adapter.setValue(
                  'config.loop_variables',
                  variables.map((entry, i) =>
                    i === index ? { ...entry, value_type } : entry
                  )
                )
              }
            />
            <Button
              aria-label={`${t('containers.delete_variable')} ${index + 1}`}
              type="text"
              icon={<DeleteOutlined />}
              onClick={() => {
                rowKeys.removeItemKey(index);
                adapter.setValue(
                  'config.loop_variables',
                  variables.filter((_, i) => i !== index)
                );
              }}
            />
          </Flex>
          <Typography.Text type="secondary">
            {t('containers.initial_value')}
          </Typography.Text>
          <ContainerValueField
            required
            label={`${t('containers.initial_value')} ${variable.key}`}
            valueType={variable.value_type}
            expression={
              initial.find((entry) => entry.name === variable.key)?.value ??
              null
            }
            options={optionsFor(adapter, 'bindings.variables')}
            onChange={(value) =>
              updateBinding('bindings.variables', variable.key, value)
            }
          />
          <Typography.Text type="secondary">
            {t('containers.success_update')}
          </Typography.Text>
          <ContainerValueField
            label={`${t('containers.success_update')} ${variable.key}`}
            valueType={variable.value_type}
            expression={
              updates.find((entry) => entry.name === variable.key)?.value ??
              null
            }
            options={optionsFor(adapter, 'bindings.variable_updates')}
            onChange={(value) =>
              updateBinding('bindings.variable_updates', variable.key, value)
            }
          />
        </div>
      ))}
      <Button
        onClick={() => {
          let index = variables.length + 1;
          while (
            variables.some((variable) => variable.key === `variable_${index}`)
          )
            index++;
          rowKeys.insertItemKey();
          adapter.setValue('config.loop_variables', [
            ...variables,
            { key: `variable_${index}`, value_type: 'string' }
          ]);
        }}
      >
        {t('containers.add_variable')}
      </Button>
    </div>
  );
}
export function ContainerSelectorField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const binding = adapter.getValue(block.path) as FlowBinding | undefined;
  const flatten = adapter.getValue('config.flatten_output') === true;
  const arrayOnly =
    block.path === 'bindings.items' ||
    (block.path === 'bindings.output' && flatten);
  const options = optionsFor(adapter, block.path).filter(
    (option) =>
      !arrayOnly ||
      option.valueType === 'json' ||
      option.valueType.startsWith('array')
  );
  return (
    <SelectorField
      ariaLabel={block.label}
      options={options}
      value={binding?.kind === 'selector' ? binding.value : []}
      onChange={(value) =>
        adapter.setValue(
          block.path,
          (value as string[]).length ? { kind: 'selector', value } : null
        )
      }
    />
  );
}
export function ContainerBreakConditionField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const { t } = useTranslation('agentFlow');
  const binding = adapter.getValue(block.path) as FlowBinding | undefined;
  const value: FlowConditionGroupDocument =
    binding?.kind === 'condition_group'
      ? binding.value
      : { operator: 'and', conditions: [] };
  return (
    <div className="agent-flow-container-field">
      <Typography.Text type="secondary">
        {t('containers.break_condition_help')}
      </Typography.Text>
      <ConditionGroupField
        ariaLabel={block.label}
        options={optionsFor(adapter, block.path)}
        value={value}
        onChange={(value) =>
          adapter.setValue(
            block.path,
            value.conditions.length ? { kind: 'condition_group', value } : null
          )
        }
      />
      {binding && (
        <Button type="text" onClick={() => adapter.setValue(block.path, null)}>
          {t('auto.clear')}
        </Button>
      )}
    </div>
  );
}
export function ContainerPositiveIntegerField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const { t } = useTranslation('agentFlow');
  const value = adapter.getValue(block.path);
  const timeout = block.path === 'config.round_timeout_ms';
  return (
    <div className="agent-flow-container-field">
      <InputNumber
        aria-label={block.label}
        min={timeout ? undefined : 1}
        step={1}
        precision={timeout ? undefined : 0}
        placeholder={timeout ? t('containers.timeout_optional') : undefined}
        value={
          typeof value === 'number' ? (timeout ? value / 60000 : value) : null
        }
        onChange={(value) =>
          adapter.setValue(
            block.path,
            timeout && value !== null
              ? Math.round(Number(value) * 60000)
              : value
          )
        }
      />
      {block.path === 'config.round_timeout_ms' && (
        <Typography.Text type="secondary">
          {t('containers.timeout_help')}
        </Typography.Text>
      )}
    </div>
  );
}
export function ContainerParallelismField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const { t } = useTranslation('agentFlow');
  const value = adapter.getValue(block.path);
  const parallelism = typeof value === 'number' ? value : 1;
  return (
    <Flex gap={12} align="center">
      <Switch
        aria-label={t('containers.parallel_enabled')}
        checked={parallelism > 1}
        onChange={(checked) => adapter.setValue(block.path, checked ? 2 : 1)}
      />
      <InputNumber
        aria-label={block.label}
        min={1}
        precision={0}
        step={1}
        value={parallelism}
        onChange={(value) => adapter.setValue(block.path, value)}
      />
    </Flex>
  );
}
