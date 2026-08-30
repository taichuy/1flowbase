import { CloseOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Form, Input, Modal, Popover, Space } from 'antd';
import type { FormInstance } from 'antd';

import { i18nText } from '../../../../shared/i18n/text';
import { PageTreeIconPicker } from '../../lib/page-tree-icons/PageTreeIconPicker';
import { PageTreeIcon } from '../../lib/page-tree-icons/registry';

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
      nodeKind: 'group' | 'page';
      showSlug?: boolean;
    }
  | {
      kind: 'tooltip';
      nodeId: string;
      title: string;
      initialTooltip: string;
    };

function renderPageTreeIconPicker(
  selectedIcon: string | undefined,
  onChange: (icon: string | undefined) => void,
  iconPickerOpen: boolean,
  onIconPickerOpenChange: (open: boolean) => void
) {
  const picker = (
    <div className="frontstage-page-tree-form__icon-popover">
      <PageTreeIconPicker
        selectedIcon={selectedIcon}
        onSelect={(iconName) => {
          onChange(iconName);
          onIconPickerOpenChange(false);
        }}
      />
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
          aria-label={i18nText('frontstage', 'auto.select_icon')}
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
          <PageTreeIcon name={selectedIcon} fallback={<PlusOutlined />} />
        </button>
      </Popover>
      {selectedIcon ? (
        <button
          aria-label={i18nText('frontstage', 'auto.clear_icon')}
          className="frontstage-page-tree-form__icon-clear-button"
          type="button"
          onClick={() => onChange(undefined)}
        >
          <CloseOutlined />
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
      okText={i18nText('frontstage', 'auto.confirm')}
      cancelText={i18nText('frontstage', 'auto.cancel')}
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
          <Form.Item
            label={i18nText('frontstage', 'auto.description')}
            name="tooltip"
          >
            <Input.TextArea autoSize={{ minRows: 3, maxRows: 6 }} />
          </Form.Item>
        ) : (
          <>
            <Form.Item
              label={i18nText('frontstage', 'auto.name')}
              name="title"
              rules={[
                {
                  required: true,
                  whitespace: true,
                  message: i18nText('frontstage', 'auto.name_required')
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
            <Form.Item label={i18nText('frontstage', 'auto.icon')} name="icon">
              <PageTreeIconPickerField
                iconPickerOpen={iconPickerOpen}
                onIconPickerOpenChange={onIconPickerOpenChange}
              />
            </Form.Item>
            <Form.Item
              label={i18nText('frontstage', 'auto.description')}
              name="tooltip"
            >
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
