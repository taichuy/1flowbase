import * as AntIcons from '@ant-design/icons';
import { Button, Form, Input, Modal, Popover, Space } from 'antd';
import type { FormInstance } from 'antd';
import type { ElementType } from 'react';

import { i18nText } from '../../../../shared/i18n/text';

type PageTreeFormValues = {
  title?: string;
  icon?: string;
  tooltip?: string;
  slug?: string;
};

type PageTreeFormDialog =
  | {
      kind: 'create';
      nodeKind: 'group' | 'page';
      parentId: string | null;
      rank: string;
      title: string;
      initialTitle: string;
      initialIcon: string;
      initialTooltip: string;
      initialSlug?: string;
      showSlug?: boolean;
    }
  | {
      kind: 'rename';
      nodeId: string;
      title: string;
      initialTitle: string;
      initialIcon: string;
      initialTooltip: string;
      initialSlug?: string;
      showSlug?: boolean;
    }
  | {
      kind: 'tooltip';
      nodeId: string;
      title: string;
      initialTooltip: string;
    };

type AntIconComponent = ElementType<{ className?: string }>;

const antIconComponents = AntIcons as Record<string, unknown>;
const pageTreeIconEntries = Object.entries(antIconComponents)
  .filter(
    (entry): entry is [string, AntIconComponent] =>
      /(?:Outlined|Filled|TwoTone)$/.test(entry[0]) &&
      (typeof entry[1] === 'function' ||
        (typeof entry[1] === 'object' && entry[1] !== null))
  )
  .sort(([left], [right]) => left.localeCompare(right));
const pageTreeIconMap = Object.fromEntries(pageTreeIconEntries);
const CloseIcon =
  (pageTreeIconMap.CloseOutlined as AntIconComponent | undefined) ??
  (() => null);
const PlusIcon =
  (pageTreeIconMap.PlusOutlined as AntIconComponent | undefined) ??
  (() => null);

function renderPageTreeIconPicker(
  selectedIcon: string | undefined,
  onChange: (icon: string | undefined) => void,
  iconPickerOpen: boolean,
  onIconPickerOpenChange: (open: boolean) => void
) {
  const SelectedIcon = selectedIcon
    ? (pageTreeIconMap[selectedIcon] as AntIconComponent | undefined)
    : undefined;
  const DisplayIcon = SelectedIcon ?? PlusIcon;
  const picker = (
    <div className="frontstage-page-tree-form__icon-popover">
      <div className="frontstage-page-tree-form__icon-grid">
        {pageTreeIconEntries.map(([iconName, Icon]) => (
          <button
            key={iconName}
            aria-label={iconName}
            className={[
              'frontstage-page-tree-form__icon-button',
              selectedIcon === iconName
                ? 'frontstage-page-tree-form__icon-button--selected'
                : null
            ]
              .filter(Boolean)
              .join(' ')}
            type="button"
            onClick={() => {
              onChange(iconName);
              onIconPickerOpenChange(false);
            }}
          >
            <Icon />
          </button>
        ))}
      </div>
    </div>
  );

  return (
    <div className="frontstage-page-tree-form__icon-field">
      <Popover
        arrow={false}
        content={picker}
        open={iconPickerOpen}
        placement="bottomLeft"
        trigger="click"
        onOpenChange={onIconPickerOpenChange}
      >
        <button
          aria-label={i18nText("frontstage", "auto.select_icon")}
          className={[
            'frontstage-page-tree-form__icon-select-button',
            selectedIcon
              ? 'frontstage-page-tree-form__icon-select-button--with-clear'
              : null
          ]
            .filter(Boolean)
            .join(' ')}
          type="button"
        >
          <DisplayIcon />
        </button>
      </Popover>
      {selectedIcon ? (
        <button
          aria-label={i18nText("frontstage", "auto.clear_icon")}
          className="frontstage-page-tree-form__icon-clear-button"
          type="button"
          onClick={() => onChange(undefined)}
        >
          <CloseIcon />
        </button>
      ) : null}
    </div>
  );
}

function PageTreeIconPickerField({
  value,
  onChange,
  iconPickerOpen,
  onIconPickerOpenChange
}: {
  value?: string;
  onChange?: (icon: string | undefined) => void;
  iconPickerOpen: boolean;
  onIconPickerOpenChange: (open: boolean) => void;
}) {
  return renderPageTreeIconPicker(
    value,
    (icon) => onChange?.(icon),
    iconPickerOpen,
    onIconPickerOpenChange
  );
}

type PageTreeFormModalProps = {
  dialog: PageTreeFormDialog | null;
  form: FormInstance<PageTreeFormValues>;
  iconPickerOpen: boolean;
  isOperationPending: boolean;
  onCancel: () => void;
  onIconPickerOpenChange: (open: boolean) => void;
  onRefreshSlug?: () => void;
  onSubmit: () => void;
};

function PageTreeFormModal({
  dialog,
  form,
  iconPickerOpen,
  isOperationPending,
  onCancel,
  onIconPickerOpenChange,
  onRefreshSlug,
  onSubmit
}: PageTreeFormModalProps) {
  return (
    <Modal
      title={dialog?.title}
      open={Boolean(dialog)}
      okText={i18nText("frontstage", "auto.confirm")}
      cancelText={i18nText("frontstage", "auto.cancel")}
      confirmLoading={isOperationPending}
      destroyOnHidden
      forceRender
      onCancel={onCancel}
      onOk={() => form.submit()}
    >
      <Form<PageTreeFormValues>
        form={form}
        layout="vertical"
        preserve={false}
        onFinish={onSubmit}
      >
        {dialog?.kind === 'tooltip' ? (
          <Form.Item label={i18nText("frontstage", "auto.description")} name="tooltip">
            <Input.TextArea autoSize={{ minRows: 3, maxRows: 6 }} />
          </Form.Item>
        ) : (
          <>
            <Form.Item
              label={i18nText("frontstage", "auto.name")}
              name="title"
              rules={[
                {
                  required: true,
                  whitespace: true,
                  message: i18nText("frontstage", "auto.name_required")
                }
              ]}
            >
              <Input autoFocus />
            </Form.Item>
            {dialog?.kind === 'create' && dialog.showSlug ? (
              <Form.Item label="访问路径" required>
                <Space.Compact style={{ width: '100%' }}>
                  <Form.Item
                    name="slug"
                    noStyle
                    rules={[
                      {
                        required: true,
                        whitespace: true,
                        message: '访问路径不能为空'
                      }
                    ]}
                  >
                    <Input aria-label="访问路径" prefix="/" />
                  </Form.Item>
                  <Button aria-label="刷新访问路径" onClick={onRefreshSlug}>
                    刷新
                  </Button>
                </Space.Compact>
              </Form.Item>
            ) : null}
            <Form.Item label={i18nText("frontstage", "auto.icon")} name="icon">
              <PageTreeIconPickerField
                iconPickerOpen={iconPickerOpen}
                onIconPickerOpenChange={onIconPickerOpenChange}
              />
            </Form.Item>
            <Form.Item label={i18nText("frontstage", "auto.description")} name="tooltip">
              <Input.TextArea autoSize={{ minRows: 3, maxRows: 6 }} />
            </Form.Item>
          </>
        )}
      </Form>
    </Modal>
  );
}

export type { PageTreeFormDialog, PageTreeFormValues };
export { PageTreeFormModal };
