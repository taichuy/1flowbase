import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Empty,
  Form,
  Input,
  Modal,
  Popconfirm,
  Space,
  Spin,
  Tag,
  Tooltip
} from 'antd';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import EditOutlined from '@ant-design/icons/es/icons/EditOutlined';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import ArrowUpOutlined from '@ant-design/icons/es/icons/ArrowUpOutlined';
import ArrowDownOutlined from '@ant-design/icons/es/icons/ArrowDownOutlined';
import { useTranslation } from 'react-i18next';
import {
  fetchFrontstagePageTabs,
  frontstagePageTabsQueryKey,
  type FrontstagePageTab
} from '../../../../frontstage/api/page-tabs';
import {
  createTabRankUpdates,
  nextRank
} from '../../../../frontstage/lib/page-tab-order';
import { useRouteManagementMutation } from '../../../api/ui-management/route-management';

export function RoutePageTabs({
  workspaceId,
  pageId,
  editable,
  presentation
}: {
  workspaceId: string;
  pageId: string;
  editable: boolean;
  presentation: 'single' | 'tabs';
}) {
  const { t } = useTranslation('settingsUiManagement');
  const query = useQuery({
    queryKey: frontstagePageTabsQueryKey(workspaceId, pageId),
    queryFn: () => fetchFrontstagePageTabs(workspaceId, pageId),
    retry: false
  });
  const mutation = useRouteManagementMutation(workspaceId);
  const [editor, setEditor] = useState<FrontstagePageTab | 'new' | null>(null);
  const [form] = Form.useForm<{ title: string; route_segment: string }>();
  const tabs = query.data ?? [];
  const pending = mutation.isPending;
  const run = async (action: Parameters<typeof mutation.mutateAsync>[0]) => {
    if (!editable || pending) return false;
    try {
      await mutation.mutateAsync(action);
      return true;
    } catch {
      return false;
    }
  };
  const save = async (values: { title: string; route_segment: string }) => {
    if (!editor || (editor === 'new' && presentation !== 'tabs')) return;
    const ok = await run(
      editor === 'new'
        ? {
            kind: 'create-tab',
            pageId,
            input: {
              title: values.title.trim(),
              route_segment: values.route_segment.trim(),
              rank: nextRank(tabs)
            }
          }
        : {
            kind: 'update-tab',
            pageId,
            id: editor.id,
            input: { title: values.title.trim() }
          }
    );
    if (ok) setEditor(null);
  };
  const open = (tab: FrontstagePageTab | 'new') => {
    mutation.reset();
    setEditor(tab);
    form.setFieldsValue({
      title: tab === 'new' ? '' : (tab.title ?? ''),
      route_segment: tab === 'new' ? '' : (tab.route_segment ?? '')
    });
  };
  return (
    <div className="route-page-tabs">
      <div className="route-page-tabs__toolbar">
        <span>{t('routes_tabs_hint')}</span>
        {editable ? (
          <Button
            icon={<PlusOutlined />}
            onClick={() => open('new')}
            disabled={pending || presentation !== 'tabs'}
          >
            {t('routes_add_tab')}
          </Button>
        ) : null}
      </div>
      {presentation !== 'tabs' ? (
        <Alert type="info" showIcon title={t('routes_tabs_disabled')} />
      ) : null}
      {mutation.isError && !editor ? (
        <Alert type="error" showIcon title={t('routes_action_failed')} />
      ) : null}
      {query.isPending ? (
        <Spin />
      ) : query.isError ? (
        <Alert
          type="error"
          title={t('routes_load_failed')}
          action={
            <Button onClick={() => void query.refetch()}>
              {t('routes_retry')}
            </Button>
          }
        />
      ) : !tabs.length ? (
        <Empty description={t('routes_tabs_empty')} />
      ) : (
        <div className="route-page-tabs__list">
          {tabs.map((tab, index) => (
            <div className="route-page-tabs__row" key={tab.id}>
              <div className="route-page-tabs__label">
                <strong>{tab.title || t('routes_unnamed_tab')}</strong>
                {tab.is_default ? <Tag>{t('routes_default_tab')}</Tag> : null}
                <span>
                  {tab.route_segment
                    ? `/${tab.route_segment}`
                    : t('routes_default_path')}
                </span>
              </div>
              {editable ? (
                <Space size={2}>
                  <Tooltip title={t('routes_up')}>
                    <Button
                      type="text"
                      icon={<ArrowUpOutlined />}
                      aria-label={`${t('routes_up')} ${tab.title ?? ''}`}
                      disabled={pending || index === 0}
                      onClick={() =>
                        void run({
                          kind: 'reorder-tabs',
                          pageId,
                          updates: createTabRankUpdates(
                            tabs,
                            tab.id,
                            tabs[index - 1].id
                          )
                        })
                      }
                    />
                  </Tooltip>
                  <Tooltip title={t('routes_down')}>
                    <Button
                      type="text"
                      icon={<ArrowDownOutlined />}
                      aria-label={`${t('routes_down')} ${tab.title ?? ''}`}
                      disabled={pending || index === tabs.length - 1}
                      onClick={() =>
                        void run({
                          kind: 'reorder-tabs',
                          pageId,
                          updates: createTabRankUpdates(
                            tabs,
                            tab.id,
                            tabs[index + 1].id
                          )
                        })
                      }
                    />
                  </Tooltip>
                  <Tooltip title={t('routes_rename')}>
                    <Button
                      type="text"
                      icon={<EditOutlined />}
                      aria-label={`${t('routes_rename')} ${tab.title ?? ''}`}
                      disabled={pending}
                      onClick={() => open(tab)}
                    />
                  </Tooltip>
                  <Popconfirm
                    title={t('routes_delete_tab_confirm')}
                    description={t('routes_delete_tab_description')}
                    onConfirm={() =>
                      run({ kind: 'delete-tab', pageId, id: tab.id })
                    }
                    disabled={pending || tabs.length <= 1}
                  >
                    <Button
                      type="text"
                      danger
                      icon={<DeleteOutlined />}
                      aria-label={`${t('delete')} ${tab.title ?? ''}`}
                      disabled={pending || tabs.length <= 1}
                    />
                  </Popconfirm>
                </Space>
              ) : null}
            </div>
          ))}
        </div>
      )}
      <Modal
        open={Boolean(editor)}
        title={editor === 'new' ? t('routes_add_tab') : t('routes_rename')}
        onCancel={() => {
          if (!pending) setEditor(null);
        }}
        onOk={() => form.submit()}
        confirmLoading={pending}
        destroyOnHidden
      >
        {mutation.isError ? (
          <Alert type="error" title={t('routes_action_failed')} />
        ) : null}
        <Form
          name="route-tab-settings"
          form={form}
          layout="vertical"
          onFinish={(values) => void save(values)}
        >
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
          {editor === 'new' ? (
            <Form.Item
              name="route_segment"
              label={t('routes_tab_path')}
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
        </Form>
      </Modal>
    </div>
  );
}
