import type { NodeRuntimeUiContract } from '@1flowbase/flow-schema';
import {
  createNodeRuntimeContract,
  panelField,
  panelSection,
  basicsPanelSection
} from '../../contract-builders';
import { i18nText } from '../../../../../../shared/i18n/text';

export function createContainerContract(
  type: 'loop' | 'iteration'
): NodeRuntimeUiContract {
  const loop = type === 'loop';
  return createNodeRuntimeContract({
    type,
    title: loop
      ? i18nText('agentFlow', 'containers.loop')
      : i18nText('agentFlow', 'containers.iteration'),
    description: loop
      ? i18nText('agentFlow', 'auto.conditionally_execute_node_repeatedly')
      : i18nText('agentFlow', 'auto.iterate_through_list_process_each_item'),
    category: 'control',
    config: loop
      ? { max_rounds: 10, error_policy: 'fail', loop_variables: [] }
      : { parallelism: 1, flatten_output: false, error_policy: 'fail' },
    outputs: [
      {
        key: 'result',
        title: i18nText('agentFlow', 'auto.aggregate_output'),
        valueType: 'array'
      },
      ...(loop
        ? [
            {
              key: 'variables',
              title: i18nText('agentFlow', 'containers.variables'),
              valueType: 'object'
            }
          ]
        : [])
    ],
    panelSections: [
      basicsPanelSection,
      panelSection(
        'inputs',
        'Inputs',
        loop
          ? [
              panelField({
                key: 'config.loop_variables',
                title: i18nText('agentFlow', 'containers.variables'),
                renderer: 'container_variables',
                valueType: 'array'
              }),
              panelField({
                key: 'bindings.break_condition',
                title: i18nText('agentFlow', 'containers.break_condition'),
                renderer: 'container_break_condition',
                valueType: 'json'
              }),
              panelField({
                key: 'config.max_rounds',
                title: i18nText('agentFlow', 'auto.maximum_number_rounds'),
                renderer: 'container_positive_integer',
                valueType: 'number',
                required: true,
                min: 1,
                step: 1
              }),
              panelField({
                key: 'bindings.output',
                title: i18nText('agentFlow', 'containers.output'),
                renderer: 'container_selector'
              })
            ]
          : [
              panelField({
                key: 'bindings.items',
                title: i18nText('agentFlow', 'containers.items'),
                renderer: 'container_selector',
                required: true,
                valueType: 'array'
              }),
              panelField({
                key: 'bindings.output',
                title: i18nText('agentFlow', 'containers.output'),
                renderer: 'container_selector',
                required: true
              }),
              panelField({
                key: 'config.parallelism',
                title: i18nText('agentFlow', 'containers.parallelism'),
                renderer: 'container_parallelism',
                valueType: 'number',
                min: 1,
                step: 1
              }),
              panelField({
                key: 'config.flatten_output',
                title: i18nText('agentFlow', 'containers.flatten_output'),
                renderer: 'switch',
                valueType: 'boolean'
              })
            ]
      ),
      panelSection('policy', i18nText('agentFlow', 'containers.execution'), [
        panelField({
          key: 'config.round_timeout_ms',
          title: i18nText('agentFlow', 'containers.round_timeout_ms'),
          renderer: 'container_positive_integer',
          valueType: 'number',
          required: true,
          min: 1,
          step: 1
        }),
        panelField({
          key: 'config.error_policy',
          title: i18nText('agentFlow', 'containers.error_policy'),
          renderer: 'static_select',
          options: [
            { label: i18nText('agentFlow', 'containers.fail'), value: 'fail' },
            {
              label: i18nText('agentFlow', 'containers.break'),
              value: 'break'
            },
            {
              label: i18nText('agentFlow', 'containers.continue'),
              value: 'continue'
            }
          ]
        })
      ])
    ]
  });
}

export function createContainerStartContract() {
  return createNodeRuntimeContract({
    type: 'container_start',
    title: i18nText('agentFlow', 'containers.start'),
    description: '',
    category: 'control',
    config: {},
    outputs: [],
    panelSections: []
  });
}
