import { useDataBlockPopupContainer } from '../shared/useDataBlockPopupContainer';
import { DataBlockModal as Modal } from '../shared/DataBlockModal';
import { useState } from 'react';
import {
  Button,
  Dropdown,
  Form,
  Input,
  InputNumber,
  Select,
  Space,
  Switch
} from 'antd';
import { SettingOutlined } from '@ant-design/icons';
import { i18nText } from '../../../../../shared/i18n/text';
import type { DataBlockTarget } from '../../../lib/data-blocks/host-context';
import type {
  DataBlockAction,
  DataBlockColumn,
  DataBlockField,
  TableBlockConfig
} from '../../../lib/data-blocks/types';
import { DataRelationEditor } from '../shared/DataRelationEditor';
import { DataBlockFilterEditor } from '../shared/DataBlockFilterEditor';
import { presentationOptions, sizeOptions } from './table-labels';

export type SaveTableConfig = (config: TableBlockConfig) => Promise<boolean>;

export function TableBlockSettings({
  config,
  fields,
  targets,
  blockId,
  disabled,
  onSave
}: {
  config: TableBlockConfig;
  fields: DataBlockField[];
  targets: readonly DataBlockTarget[];
  blockId: string;
  disabled: boolean;
  onSave: SaveTableConfig;
}) {
  const popupContainer = useDataBlockPopupContainer();
  const [section, setSection] = useState<string | null>(null);
  const [draft, setDraft] = useState(config);
  const items = [
    {
      key: 'title',
      label: i18nText('frontstage', 'data_blocks.form.title_description')
    },
    {
      key: 'layout',
      label: i18nText('frontstage', 'data_blocks.table.layout')
    },
    {
      key: 'filter',
      label: i18nText('frontstage', 'data_blocks.table.fixed_filter')
    },
    {
      key: 'sort',
      label: i18nText('frontstage', 'data_blocks.table.default_sort')
    },
    {
      key: 'relation',
      label: i18nText('frontstage', 'data_blocks.table.relation')
    },
    {
      key: 'linkage',
      label: i18nText('frontstage', 'data_blocks.table.linkage')
    }
  ];
  const fieldOptions = fields.map((field) => ({
    label: field.title,
    value: field.code
  }));
  return (
    <>
      <Dropdown
        trigger={['hover', 'click']}
        getPopupContainer={popupContainer}
        menu={{
          items,
          onClick: ({ key }) => {
            setDraft(config);
            setSection(key);
          }
        }}
      >
        <Button
          className="data-table-design-button"
          size="small"
          icon={<SettingOutlined />}
          disabled={disabled}
          aria-label={i18nText('frontstage', 'auto.configuration')}
        />
      </Dropdown>
      <Modal
        getContainer={false}
        open={section !== null}
        title={items.find((item) => item.key === section)?.label}
        onCancel={() => setSection(null)}
        confirmLoading={disabled}
        okButtonProps={{
          disabled: Boolean(draft.linkage && !draft.linkage.field_code)
        }}
        okText={i18nText('frontstage', 'files.save')}
        cancelText={i18nText('frontstage', 'auto.cancel')}
        onOk={async () => {
          if (await onSave(draft)) setSection(null);
        }}
      >
        <Form layout="vertical" disabled={disabled}>
          {section === 'title' && (
            <>
              <Form.Item label={i18nText('frontstage', 'auto.title')}>
                <Input
                  value={draft.title}
                  onChange={(event) =>
                    setDraft({ ...draft, title: event.target.value })
                  }
                />
              </Form.Item>
              <Form.Item label={i18nText('frontstage', 'auto.description')}>
                <Input.TextArea
                  value={draft.description}
                  onChange={(event) =>
                    setDraft({ ...draft, description: event.target.value })
                  }
                />
              </Form.Item>
            </>
          )}
          {section === 'layout' && (
            <>
              <Form.Item
                label={i18nText('frontstage', 'data_blocks.table.height')}
              >
                <InputNumber
                  min={120}
                  max={4000}
                  value={draft.height}
                  onChange={(height) => setDraft({ ...draft, height })}
                />
              </Form.Item>
              <Form.Item
                label={i18nText('frontstage', 'data_blocks.table.page_size')}
              >
                <Select
                  value={draft.page_size}
                  options={[10, 20, 50, 100].map((value) => ({
                    label: value,
                    value
                  }))}
                  onChange={(page_size) => setDraft({ ...draft, page_size })}
                />
              </Form.Item>
              <Form.Item
                label={i18nText('frontstage', 'data_blocks.table.density')}
              >
                <Select
                  value={draft.density}
                  options={sizeOptions()}
                  onChange={(density) => setDraft({ ...draft, density })}
                />
              </Form.Item>
              <Form.Item
                label={i18nText('frontstage', 'data_blocks.table.show_index')}
              >
                <Switch
                  checked={draft.show_index}
                  onChange={(show_index) => setDraft({ ...draft, show_index })}
                />
              </Form.Item>
            </>
          )}
          {section === 'filter' && (
            <DataBlockFilterEditor
              value={draft.filter}
              onChange={(filter) => setDraft({ ...draft, filter })}
              fields={fields}
            />
          )}
          {section === 'sort' && (
            <>
              <Form.Item
                label={i18nText('frontstage', 'data_blocks.table.sort_field')}
              >
                <Select
                  allowClear
                  value={draft.sort?.field}
                  options={fieldOptions}
                  onChange={(field) =>
                    setDraft({
                      ...draft,
                      sort: field
                        ? { field, direction: draft.sort?.direction ?? 'asc' }
                        : null
                    })
                  }
                />
              </Form.Item>
              {draft.sort && (
                <Form.Item
                  label={i18nText(
                    'frontstage',
                    'data_blocks.table.sort_direction'
                  )}
                >
                  <Select
                    value={draft.sort.direction}
                    options={[
                      {
                        value: 'asc',
                        label: i18nText(
                          'frontstage',
                          'data_blocks.table.ascending'
                        )
                      },
                      {
                        value: 'desc',
                        label: i18nText(
                          'frontstage',
                          'data_blocks.table.descending'
                        )
                      }
                    ]}
                    onChange={(direction) =>
                      setDraft({
                        ...draft,
                        sort: { field: draft.sort!.field, direction }
                      })
                    }
                  />
                </Form.Item>
              )}
            </>
          )}
          {section === 'relation' && (
            <DataRelationEditor
              value={draft.relation}
              fields={fields}
              onChange={(relation) => setDraft({ ...draft, relation })}
            />
          )}
          {section === 'linkage' && (
            <>
              <Form.Item
                label={i18nText('frontstage', 'data_blocks.table.source_block')}
              >
                <Select
                  allowClear
                  value={draft.linkage?.source_block_id}
                  options={targets
                    .filter((target) => target.block_id !== blockId)
                    .map((target) => ({
                      label: target.title,
                      value: target.block_id
                    }))}
                  onChange={(source_block_id) =>
                    setDraft({
                      ...draft,
                      linkage: source_block_id
                        ? {
                            source_block_id,
                            source_output:
                              draft.linkage?.source_output ?? 'record_id',
                            field_code:
                              draft.linkage?.field_code ?? fields[0]?.code ?? ''
                          }
                        : undefined
                    })
                  }
                />
              </Form.Item>
              {draft.linkage && (
                <>
                  <Form.Item
                    label={i18nText(
                      'frontstage',
                      'data_blocks.table.source_output'
                    )}
                  >
                    <Select
                      value={draft.linkage.source_output}
                      options={[
                        {
                          value: 'record_id',
                          label: i18nText(
                            'frontstage',
                            'data_blocks.table.current_record'
                          )
                        },
                        {
                          value: 'selected_record_ids',
                          label: i18nText(
                            'frontstage',
                            'data_blocks.table.selected_records'
                          )
                        }
                      ]}
                      onChange={(source_output) =>
                        setDraft({
                          ...draft,
                          linkage: { ...draft.linkage!, source_output }
                        })
                      }
                    />
                  </Form.Item>
                  <Form.Item
                    label={i18nText(
                      'frontstage',
                      'data_blocks.table.target_field'
                    )}
                  >
                    <Select
                      value={draft.linkage.field_code}
                      options={fieldOptions}
                      onChange={(field_code) =>
                        setDraft({
                          ...draft,
                          linkage: { ...draft.linkage!, field_code }
                        })
                      }
                    />
                  </Form.Item>
                </>
              )}
            </>
          )}
        </Form>
      </Modal>
    </>
  );
}

export function TableColumnSettings({
  column,
  title,
  disabled,
  onSave
}: {
  column: DataBlockColumn;
  title: string;
  disabled: boolean;
  onSave: (column: DataBlockColumn) => Promise<boolean>;
}) {
  const popupContainer = useDataBlockPopupContainer();
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(column);
  return (
    <>
      <Dropdown
        getPopupContainer={popupContainer}
        trigger={['hover', 'click']}
        menu={{
          items: [
            {
              key: 'edit',
              label: i18nText('frontstage', 'data_blocks.table.column_settings')
            },
            {
              key: 'hide',
              label: i18nText('frontstage', 'data_blocks.table.hide_column')
            }
          ],
          onClick: ({ key, domEvent }) => {
            domEvent.stopPropagation();
            if (key === 'hide') void onSave({ ...column, hidden: true });
            else {
              setDraft(column);
              setOpen(true);
            }
          }
        }}
      >
        <Button
          className="data-table-column-config"
          size="small"
          type="text"
          icon={<SettingOutlined />}
          disabled={disabled}
          aria-label={`${i18nText('frontstage', 'data_blocks.table.column_settings')} ${title}`}
          onClick={(event) => event.stopPropagation()}
        />
      </Dropdown>
      <Modal
        getContainer={false}
        open={open}
        title={title}
        onCancel={() => setOpen(false)}
        confirmLoading={disabled}
        okText={i18nText('frontstage', 'files.save')}
        cancelText={i18nText('frontstage', 'auto.cancel')}
        onOk={async () => {
          if (await onSave(draft)) setOpen(false);
        }}
      >
        <Form layout="vertical" disabled={disabled}>
          <Form.Item
            label={i18nText('frontstage', 'data_blocks.table.column_title')}
          >
            <Input
              value={draft.title}
              onChange={(event) =>
                setDraft({ ...draft, title: event.target.value })
              }
            />
          </Form.Item>
          <Form.Item label={i18nText('frontstage', 'data_blocks.form.tooltip')}>
            <Input
              value={draft.tooltip}
              onChange={(event) =>
                setDraft({ ...draft, tooltip: event.target.value })
              }
            />
          </Form.Item>
          <Form.Item label={i18nText('frontstage', 'data_blocks.table.width')}>
            <InputNumber
              min={60}
              max={2000}
              value={draft.width}
              onChange={(width) =>
                setDraft({ ...draft, width: width ?? undefined })
              }
            />
          </Form.Item>
          <Form.Item label={i18nText('frontstage', 'data_blocks.table.fixed')}>
            <Select
              allowClear
              value={draft.fixed}
              options={[
                {
                  value: 'left',
                  label: i18nText('frontstage', 'data_blocks.table.left')
                },
                {
                  value: 'right',
                  label: i18nText('frontstage', 'data_blocks.table.right')
                }
              ]}
              onChange={(fixed) => setDraft({ ...draft, fixed })}
            />
          </Form.Item>
          <Form.Item
            label={i18nText('frontstage', 'data_blocks.table.sortable')}
          >
            <Switch
              checked={draft.sortable}
              onChange={(sortable) => setDraft({ ...draft, sortable })}
            />
          </Form.Item>
          <Form.Item
            label={i18nText('frontstage', 'data_blocks.table.ellipsis')}
          >
            <Switch
              checked={draft.ellipsis}
              onChange={(ellipsis) => setDraft({ ...draft, ellipsis })}
            />
          </Form.Item>
          <Form.Item
            label={i18nText('frontstage', 'data_blocks.table.click_open')}
          >
            <Switch
              checked={draft.action?.enabled}
              onChange={(enabled) =>
                setDraft({
                  ...draft,
                  action: {
                    id: `column-${column.field_code}`,
                    kind: 'open',
                    presentation: 'drawer',
                    ...draft.action,
                    enabled
                  }
                })
              }
            />
          </Form.Item>
          {draft.action?.enabled && (
            <ActionPresentation
              action={draft.action}
              onChange={(action) => setDraft({ ...draft, action })}
            />
          )}
        </Form>
      </Modal>
    </>
  );
}

export function ActionPresentation({
  action,
  onChange
}: {
  action: DataBlockAction;
  onChange: (action: DataBlockAction) => void;
}) {
  return (
    <>
      <Form.Item
        label={i18nText('frontstage', 'data_blocks.table.presentation')}
      >
        <Select
          value={action.presentation ?? 'drawer'}
          options={presentationOptions()}
          onChange={(presentation) => onChange({ ...action, presentation })}
        />
      </Form.Item>
      <Form.Item
        label={i18nText('frontstage', 'data_blocks.table.surface_size')}
      >
        <Select
          value={action.size ?? 'middle'}
          options={sizeOptions()}
          onChange={(size) => onChange({ ...action, size })}
        />
      </Form.Item>
    </>
  );
}

export function TableActionSettings({
  action,
  disabled,
  onSave
}: {
  action: DataBlockAction;
  disabled: boolean;
  onSave: (action: DataBlockAction) => Promise<boolean>;
}) {
  const popupContainer = useDataBlockPopupContainer();
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(action);
  return (
    <>
      <Dropdown
        trigger={['hover', 'click']}
        getPopupContainer={popupContainer}
        disabled={disabled}
        menu={{
          items: [
            {
              key: 'edit',
              label: i18nText('frontstage', 'data_blocks.table.action_title')
            },
            ...(['create', 'view', 'edit', 'open'].includes(action.kind)
              ? [
                  {
                    key: 'presentation',
                    label: i18nText(
                      'frontstage',
                      'data_blocks.table.presentation'
                    ),
                    children: presentationOptions().map((option) => ({
                      key: `presentation:${option.value}`,
                      label: option.label
                    }))
                  },
                  {
                    key: 'size',
                    label: i18nText(
                      'frontstage',
                      'data_blocks.table.surface_size'
                    ),
                    children: sizeOptions().map((option) => ({
                      key: `size:${option.value}`,
                      label: option.label
                    }))
                  }
                ]
              : []),
            {
              key: 'disable',
              label: i18nText('frontstage', 'data_blocks.table.disable_action'),
              danger: true
            }
          ],
          onClick: ({ key, domEvent }) => {
            domEvent.stopPropagation();
            if (key === 'edit') {
              setDraft(action);
              setOpen(true);
            } else if (key === 'disable')
              void onSave({ ...action, enabled: false });
            else if (key.startsWith('presentation:'))
              void onSave({
                ...action,
                presentation: key.slice(13) as DataBlockAction['presentation']
              });
            else if (key.startsWith('size:'))
              void onSave({
                ...action,
                size: key.slice(5) as DataBlockAction['size']
              });
          }
        }}
      >
        <Button
          className="data-table-action-config"
          size="small"
          type="text"
          icon={<SettingOutlined />}
          disabled={disabled}
          aria-label={i18nText(
            'frontstage',
            'data_blocks.form.action_settings'
          )}
        />
      </Dropdown>
      <Modal
        getContainer={false}
        open={open}
        title={i18nText('frontstage', 'data_blocks.form.action_settings')}
        onCancel={() => setOpen(false)}
        confirmLoading={disabled}
        okText={i18nText('frontstage', 'files.save')}
        cancelText={i18nText('frontstage', 'auto.cancel')}
        onOk={async () => {
          if (await onSave(draft)) setOpen(false);
        }}
      >
        <Form layout="vertical" disabled={disabled}>
          <Form.Item
            label={i18nText('frontstage', 'data_blocks.table.action_title')}
          >
            <Input
              value={draft.title}
              onChange={(event) =>
                setDraft({ ...draft, title: event.target.value })
              }
            />
          </Form.Item>
          {['create', 'view', 'edit', 'open'].includes(action.kind) && (
            <ActionPresentation action={draft} onChange={setDraft} />
          )}
          <Space>
            <Button
              danger
              onClick={async () => {
                if (await onSave({ ...draft, enabled: false })) setOpen(false);
              }}
            >
              {i18nText('frontstage', 'data_blocks.table.disable_action')}
            </Button>
          </Space>
        </Form>
      </Modal>
    </>
  );
}
