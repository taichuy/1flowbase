import { DeleteOutlined, DragOutlined, PlusOutlined } from '@ant-design/icons';
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent
} from '@dnd-kit/core';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type {
  FlowVariableGroupDocument,
  FlowVariableGroupValueType
} from '@1flowbase/flow-schema';
import { validatePublicOutputKey } from '@1flowbase/flow-schema';
import { Button, Input, Select, Typography } from 'antd';
import { useEffect, useId, useRef, useState } from 'react';

import type { FlowSelectorOption } from '../../lib/selector-options';
import { isOutputVariableKeyAllowed } from '../../lib/output-contract/variable-key';
import { i18nText } from '../../../../shared/i18n/text';
import { SelectorField } from './SelectorField';

interface VariableGroupsFieldProps {
  ariaLabel: string;
  value: FlowVariableGroupDocument[];
  options: FlowSelectorOption[];
  onChange: (value: FlowVariableGroupDocument[]) => void;
}

const VALUE_TYPE_OPTIONS: Array<{
  label: string;
  value: FlowVariableGroupValueType;
}> = [
  { label: i18nText('agentFlow', 'auto.string'), value: 'string' },
  { label: i18nText('agentFlow', 'auto.number'), value: 'number' },
  { label: i18nText('agentFlow', 'auto.boolean'), value: 'boolean' },
  { label: i18nText('agentFlow', 'auto.object'), value: 'object' },
  { label: i18nText('agentFlow', 'auto.array'), value: 'array' }
];

function selectorEquals(left: string[], right: string[]) {
  return (
    left.length === right.length &&
    left.every((segment, index) => segment === right[index])
  );
}

function declaredSelectorType(valueType: string) {
  return valueType.startsWith('array[') ? 'array' : valueType;
}

function compatibleOptions(
  options: FlowSelectorOption[],
  valueType: FlowVariableGroupValueType
) {
  return options.filter(
    (option) => declaredSelectorType(option.valueType) === valueType
  );
}

function nextGroupKey(groups: FlowVariableGroupDocument[]) {
  const maxSuffix = groups.reduce((currentMax, group) => {
    const match = /^group(\d+)$/.exec(group.key);
    const suffix = match ? Number(match[1]) : 0;

    return Math.max(currentMax, suffix);
  }, 0);

  return `group${maxSuffix + 1}`;
}

function getGroupKeyError(
  groups: FlowVariableGroupDocument[],
  group: FlowVariableGroupDocument
) {
  if (group.key.length === 0) {
    return i18nText('agentFlow', 'auto.variable_group_key_required');
  }

  if (!isOutputVariableKeyAllowed(group.key)) {
    return i18nText('agentFlow', 'auto.variable_group_key_format_message');
  }

  if (!validatePublicOutputKey(group.key).ok) {
    return i18nText('agentFlow', 'auto.variable_group_key_reserved_message');
  }

  if (groups.filter((candidate) => candidate.key === group.key).length > 1) {
    return i18nText('agentFlow', 'auto.variable_group_keys_must_unique');
  }

  return null;
}

function replaceGroup(
  groups: FlowVariableGroupDocument[],
  index: number,
  group: FlowVariableGroupDocument
) {
  return groups.map((candidate, candidateIndex) =>
    candidateIndex === index ? group : candidate
  );
}

function SortableCandidateRow({
  id,
  ariaLabel,
  selectorAriaLabel,
  selector,
  options,
  incompatible,
  valueType,
  deleteDisabled,
  onChange,
  onDelete
}: {
  id: string;
  ariaLabel: string;
  selectorAriaLabel: string;
  selector: string[];
  options: FlowSelectorOption[];
  incompatible: boolean;
  valueType: FlowVariableGroupValueType;
  deleteDisabled: boolean;
  onChange: (selector: string[]) => void;
  onDelete: () => void;
}) {
  const {
    attributes,
    listeners,
    setActivatorNodeRef,
    setNodeRef,
    transform,
    transition,
    isDragging
  } = useSortable({ id });

  return (
    <div
      ref={setNodeRef}
      className="agent-flow-variable-groups__row"
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
        ...(isDragging ? { position: 'relative', zIndex: 1 } : {})
      }}
    >
      <Button
        ref={setActivatorNodeRef}
        aria-label={ariaLabel}
        className="agent-flow-variable-groups__drag-handle"
        icon={<DragOutlined />}
        size="small"
        type="text"
        {...attributes}
        {...listeners}
      />
      <div className="agent-flow-variable-groups__selector">
        <SelectorField
          ariaLabel={selectorAriaLabel}
          options={options}
          value={selector}
          onChange={(nextSelector) => onChange(nextSelector as string[])}
        />
        {incompatible ? (
          <Typography.Text type="danger">
            {i18nText(
              'agentFlow',
              'auto.variable_group_candidate_type_mismatch',
              { value1: valueType }
            )}
          </Typography.Text>
        ) : null}
      </div>
      <Button
        aria-label={i18nText('agentFlow', 'auto.delete_candidate')}
        className="agent-flow-variable-groups__delete-candidate"
        danger
        disabled={deleteDisabled}
        icon={<DeleteOutlined />}
        type="text"
        onClick={onDelete}
      />
    </div>
  );
}

function VariableGroupSection({
  ariaLabel,
  group,
  groupIndex,
  groups,
  options,
  onChange,
  onDelete
}: {
  ariaLabel: string;
  group: FlowVariableGroupDocument;
  groupIndex: number;
  groups: FlowVariableGroupDocument[];
  options: FlowSelectorOption[];
  onChange: (group: FlowVariableGroupDocument) => void;
  onDelete: () => void;
}) {
  const candidateIdPrefix = useId();
  const nextCandidateId = useRef(group.candidates.length);
  const [candidateIds, setCandidateIds] = useState(() =>
    group.candidates.map(
      (_, candidateIndex) => `${candidateIdPrefix}-candidate-${candidateIndex}`
    )
  );
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );
  const selectorOptions = compatibleOptions(options, group.valueType);
  const keyError = getGroupKeyError(groups, group);

  useEffect(() => {
    setCandidateIds((current) => {
      if (current.length === group.candidates.length) {
        return current;
      }

      if (current.length > group.candidates.length) {
        return current.slice(0, group.candidates.length);
      }

      const appended = [...current];
      while (appended.length < group.candidates.length) {
        appended.push(
          `${candidateIdPrefix}-candidate-${nextCandidateId.current}`
        );
        nextCandidateId.current += 1;
      }
      return appended;
    });
  }, [candidateIdPrefix, group.candidates.length]);

  function handleDragEnd({ active, over }: DragEndEvent) {
    if (!over || active.id === over.id) {
      return;
    }

    const activeIndex = candidateIds.indexOf(String(active.id));
    const overIndex = candidateIds.indexOf(String(over.id));

    if (activeIndex < 0 || overIndex < 0) {
      return;
    }

    setCandidateIds((current) => arrayMove(current, activeIndex, overIndex));
    onChange({
      ...group,
      candidates: arrayMove(group.candidates, activeIndex, overIndex)
    });
  }

  function deleteCandidate(candidateIndex: number) {
    setCandidateIds((current) =>
      current.filter((_, index) => index !== candidateIndex)
    );
    onChange({
      ...group,
      candidates: group.candidates.filter(
        (_, index) => index !== candidateIndex
      )
    });
  }

  function addCandidate() {
    setCandidateIds((current) => [
      ...current,
      `${candidateIdPrefix}-candidate-${nextCandidateId.current}`
    ]);
    nextCandidateId.current += 1;
    onChange({
      ...group,
      candidates: [...group.candidates, []]
    });
  }

  return (
    <section
      aria-label={`${ariaLabel} ${groupIndex + 1}`}
      className="agent-flow-variable-groups__item"
    >
      <div className="agent-flow-variable-groups__header">
        <div className="agent-flow-variable-groups__header-key">
          <Input
            aria-label={i18nText('agentFlow', 'auto.variable_group_key', {
              value1: groupIndex + 1
            })}
            status={keyError ? 'error' : undefined}
            value={group.key}
            onChange={(event) =>
              onChange({
                ...group,
                key: event.target.value
              })
            }
          />
          {keyError ? (
            <Typography.Text type="danger">{keyError}</Typography.Text>
          ) : null}
        </div>
        <Select
          aria-label={`${ariaLabel}-${group.key}-${i18nText('agentFlow', 'auto.type')}`}
          className="agent-flow-variable-groups__header-type"
          options={VALUE_TYPE_OPTIONS}
          value={group.valueType}
          onChange={(valueType) => onChange({ ...group, valueType })}
        />
        <Button
          aria-label={i18nText('agentFlow', 'auto.delete_variable_group', {
            value1: group.key
          })}
          className="agent-flow-variable-groups__header-delete"
          danger
          disabled={groups.length === 1}
          icon={<DeleteOutlined />}
          type="text"
          onClick={onDelete}
        />
      </div>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        modifiers={[restrictToVerticalAxis]}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={candidateIds}
          strategy={verticalListSortingStrategy}
        >
          <div className="agent-flow-variable-groups__candidates">
            {group.candidates.map((selector, candidateIndex) => {
              const selectedOption = options.find((option) =>
                selectorEquals(option.value, selector)
              );
              const incompatible =
                selector.length > 0 &&
                (!selectedOption ||
                  declaredSelectorType(selectedOption.valueType) !==
                    group.valueType);

              return (
                <SortableCandidateRow
                  key={candidateIds[candidateIndex]}
                  id={candidateIds[candidateIndex] ?? ''}
                  ariaLabel={i18nText('agentFlow', 'auto.reorder_candidate', {
                    value1: candidateIndex + 1
                  })}
                  deleteDisabled={group.candidates.length === 1}
                  incompatible={incompatible}
                  options={selectorOptions}
                  selector={selector}
                  selectorAriaLabel={`${ariaLabel}-${group.key}-${candidateIndex + 1}`}
                  valueType={group.valueType}
                  onChange={(nextSelector) =>
                    onChange({
                      ...group,
                      candidates: group.candidates.map((candidate, index) =>
                        index === candidateIndex ? nextSelector : candidate
                      )
                    })
                  }
                  onDelete={() => deleteCandidate(candidateIndex)}
                />
              );
            })}
          </div>
        </SortableContext>
      </DndContext>
      <Button
        block
        className="agent-flow-variable-groups__add-candidate"
        icon={<PlusOutlined />}
        type="dashed"
        onClick={addCandidate}
      >
        {i18nText('agentFlow', 'auto.add_candidate')}
      </Button>
    </section>
  );
}

export function VariableGroupsField({
  ariaLabel,
  value,
  options,
  onChange
}: VariableGroupsFieldProps) {
  function updateGroup(index: number, group: FlowVariableGroupDocument) {
    onChange(replaceGroup(value, index, group));
  }

  return (
    <div
      aria-label={ariaLabel}
      className="agent-flow-variable-groups"
      data-testid="variable-groups-field"
    >
      {value.map((group, groupIndex) => (
        <VariableGroupSection
          key={`variable-group-${groupIndex}`}
          ariaLabel={ariaLabel}
          group={group}
          groupIndex={groupIndex}
          groups={value}
          options={options}
          onChange={(nextGroup) => updateGroup(groupIndex, nextGroup)}
          onDelete={() =>
            onChange(value.filter((_, index) => index !== groupIndex))
          }
        />
      ))}
      <Button
        aria-label={i18nText('agentFlow', 'auto.add_variable_group')}
        block
        className="agent-flow-variable-groups__add-group"
        icon={<PlusOutlined />}
        type="dashed"
        onClick={() =>
          onChange([
            ...value,
            {
              key: nextGroupKey(value),
              valueType: 'string',
              candidates: [[]]
            }
          ])
        }
      >
        {i18nText('agentFlow', 'auto.add_variable_group')}
      </Button>
    </div>
  );
}
