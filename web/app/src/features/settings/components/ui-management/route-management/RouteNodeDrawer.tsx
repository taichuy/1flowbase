import { lazy, Suspense, useState } from 'react';
import {
  Alert,
  Button,
  Form,
  Input,
  Popover,
  Select,
  Space,
  Switch,
  Tabs,
  Tag
} from 'antd';
import FolderOutlined from '@ant-design/icons/es/icons/FolderOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import { useTranslation } from 'react-i18next';
import type { UpdateFrontstagePageNodeTitleInput } from '@1flowbase/api-client';
import type { FrontstagePageTreeNode } from '../../../../frontstage/api/page-tree';
import { PageTreeIcon } from '../../../../frontstage/lib/page-tree-icons/registry';
import { ResizableDrawer } from '../../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { useRouteManagementMutation } from '../../../api/ui-management/route-management';
import { RoutePageTabs } from './RoutePageTabs';

const IconPicker = lazy(() =>
  import('../../../../frontstage/lib/page-tree-icons/PageTreeIconPicker').then(
    (module) => ({ default: module.PageTreeIconPicker })
  )
);
export type RouteNodeEditor =
  | { kind: 'create'; nodeKind: 'group' | 'page'; parent_id: string | null }
  | { kind: 'edit'; node: FrontstagePageTreeNode };

function IconField({
  value,
  onChange
}: {
  value?: string;
  onChange?: (value: string | null) => void;
}) {
  const { t } = useTranslation('settingsUiManagement');
  const [open, setOpen] = useState(false);
  return (
    <Space>
      <Popover
        trigger="click"
        open={open}
        onOpenChange={setOpen}
        content={
          <Suspense fallback={null}>
            <IconPicker
              selectedIcon={value}
              onSelect={(name) => {
                onChange?.(name);
                setOpen(false);
              }}
            />
          </Suspense>
        }
      >
        <Button
          aria-label={t('routes_choose_icon')}
          icon={<PageTreeIcon name={value} fallback={<FolderOutlined />} />}
        >
          {t('routes_choose_icon')}
        </Button>
      </Popover>
      {value ? (
        <Button type="text" onClick={() => onChange?.(null)}>
          {t('routes_clear_icon')}
        </Button>
      ) : null}
    </Space>
  );
}

export function RouteNodeDrawer({
  editor,
  workspaceId,
  editable,
  onClose
}: {
  editor: RouteNodeEditor;
  workspaceId: string;
  editable: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation('settingsUiManagement');
  const node = editor.kind === 'edit' ? editor.node : null;
  const kind =
    node?.kind ?? (editor.kind === 'create' ? editor.nodeKind : 'page');
  const isTopbar = node
    ? node.placement === 'topbar'
    : editor.kind === 'create' && editor.parent_id === null;
  const [form] = Form.useForm<UpdateFrontstagePageNodeTitleInput>();
  const [activeTab, setActiveTab] = useState('basic');
  const mutation = useRouteManagementMutation(workspaceId);
  const pending = mutation.isPending;
  const save = async (values: UpdateFrontstagePageNodeTitleInput) => {
    if (!editable || pending) return;
    try {
      if (editor.kind === 'create') {
        await mutation.mutateAsync({
          kind: 'create',
          nodeKind: kind,
          input: {
            title: values.title?.trim(),
            icon: values.icon,
            tooltip: values.tooltip,
            parent_id: editor.parent_id,
            placement: isTopbar ? 'topbar' : 'sidebar',
            ...(isTopbar ? { slug: values.slug?.trim() } : {})
          }
        });
      } else {
        await mutation.mutateAsync({
          kind: 'update',
          id: editor.node.id,
          input: {
            title: values.title?.trim(),
            icon: values.icon ?? null,
            tooltip: values.tooltip ?? null,
            is_hidden: values.is_hidden,
            ...(kind === 'page'
              ? { content_presentation: values.content_presentation }
              : {}),
            ...(isTopbar ? { slug: values.slug?.trim() } : {})
          }
        });
      }
      onClose();
    } catch {
      /* The mutation error is shown in the drawer, retaining the form. */
    }
  };
  const initial = node
    ? {
        title: node.title,
        icon: node.icon,
        tooltip: node.tooltip,
        slug: node.slug,
        is_hidden: node.is_hidden,
        content_presentation: node.content_presentation
      }
    : {
        title: '',
        icon: null,
        tooltip: '',
        slug: `menu-${crypto.randomUUID().slice(0, 8)}`,
        is_hidden: false,
        content_presentation: 'single' as const
      };
  const basic = (
    <Form
      name="route-node-settings"
      form={form}
      initialValues={initial}
      layout="vertical"
      disabled={!editable || pending}
      onFinish={(values) => void save(values)}
    >
      {mutation.isError ? (
        <Alert type="error" showIcon title={t('routes_action_failed')} />
      ) : null}
      <Form.Item
        name="title"
        label={t('name')}
        rules={[
          {
            required: true,
            whitespace: true,
            message: t('routes_name_required')
          }
        ]}
      >
        <Input autoFocus />
      </Form.Item>
      <Form.Item name="icon" label={t('routes_icon')}>
        <IconField />
      </Form.Item>
      <Form.Item name="tooltip" label={t('routes_tooltip')}>
        <Input.TextArea rows={3} />
      </Form.Item>
      {isTopbar ? (
        <Form.Item
          name="slug"
          label={t('routes_path')}
          rules={[
            {
              required: true,
              whitespace: true,
              message: t('routes_path_required')
            }
          ]}
        >
          <Input prefix="/" />
        </Form.Item>
      ) : null}
      {editor.kind === 'edit' ? (
        <>
          <Form.Item
            name="is_hidden"
            label={t('routes_hidden')}
            valuePropName="checked"
          >
            <Switch />
          </Form.Item>
          {kind === 'page' ? (
            <Form.Item
              name="content_presentation"
              label={t('routes_presentation')}
            >
              <Select
                options={[
                  { value: 'single', label: t('routes_single') },
                  { value: 'tabs', label: t('routes_page_tabs') }
                ]}
              />
            </Form.Item>
          ) : null}
        </>
      ) : null}
    </Form>
  );
  return (
    <ResizableDrawer
      open
      title={
        editor.kind === 'create'
          ? kind === 'group'
            ? t('routes_add_group')
            : t('routes_add_page')
          : t('routes_edit_node')
      }
      ariaLabel={t('routes_edit_node')}
      resizeLabel={t('routes_resize')}
      defaultWidth={600}
      minWidth={360}
      onClose={() => {
        if (!pending) onClose();
      }}
      destroyOnClose
      rootClassName="route-node-drawer"
      footer={
        <Space>
          <Button onClick={onClose} disabled={pending}>
            {t('routes_close')}
          </Button>
          {editable && activeTab === 'basic' ? (
            <Button
              type="primary"
              loading={pending}
              onClick={() => form.submit()}
            >
              {t('routes_save')}
            </Button>
          ) : null}
        </Space>
      }
    >
      <div className="route-node-drawer__heading">
        <span className="route-node-drawer__icon">
          <PageTreeIcon
            name={node?.icon}
            fallback={
              kind === 'group' ? <FolderOutlined /> : <FileTextOutlined />
            }
          />
        </span>
        <div>
          <h3>
            {node?.title ||
              (kind === 'group' ? t('routes_add_group') : t('routes_add_page'))}
          </h3>
          <Tag>{kind === 'group' ? t('group') : t('routes_page')}</Tag>
        </div>
      </div>
      <Tabs
        activeKey={activeTab}
        onChange={setActiveTab}
        items={[
          { key: 'basic', label: t('routes_basic'), children: basic },
          ...(node?.kind === 'page'
            ? [
                {
                  key: 'tabs',
                  label: t('routes_page_tabs'),
                  children: (
                    <RoutePageTabs
                      workspaceId={workspaceId}
                      pageId={node.id}
                      presentation={node.content_presentation}
                      editable={editable}
                    />
                  )
                }
              ]
            : [])
        ]}
      />
    </ResizableDrawer>
  );
}
