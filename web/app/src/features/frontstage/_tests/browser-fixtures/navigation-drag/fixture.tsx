/* eslint-disable react-refresh/only-export-components */
import { App, ConfigProvider } from 'antd';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createRoot } from 'react-dom/client';
import { useState } from 'react';
import { TopbarNavigationItemLabel } from '../../../../../app-shell/TopbarNavigationDesigner';
import { FrontStagePageTreeSidebar } from '../../../components/FrontStagePageTreeSidebar';
import type { FrontStageTreeNode } from '../../../lib/page-tree';
import { moveNodeToTreePosition } from '../../../pages/frontstage-page/page-tree-operations';
import type { FrontstagePageTreeNode } from '../../../api/page-tree';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../../shared/i18n/app-i18n';
import '../../../../../app-shell/app-shell.css';
import '../../../../../styles/tokens.css';

const noop = () => {};
const queryClient = new QueryClient();
const topbar = ['模型日志', '报表', '设置'].map((title, index) => ({
  id: `top-${index}`,
  title,
  kind: 'group' as const,
  placement: 'topbar' as const,
  content_presentation: 'single' as const,
  children: []
})) as FrontstagePageTreeNode[];
const initialTree: FrontStageTreeNode[] = [
  {
    id: 'reports',
    kind: 'group',
    title: '报表',
    children: [
      { id: 'usage', kind: 'page', title: '模型使用报表' },
      { id: 'archive', kind: 'group', title: '历史报表', children: [] }
    ]
  },
  { id: 'settings', kind: 'group', title: '设置', children: [] },
  { id: 'apps', kind: 'page', title: '应用分组' },
  { id: 'logs', kind: 'page', title: 'AI聊天记录' }
];
function Fixture() {
  const [tree, setTree] = useState(initialTree);
  const [saves, setSaves] = useState(0);
  return (
    <ConfigProvider>
      <QueryClientProvider client={queryClient}>
        <App>
          <main
            style={{
              padding: 24,
              background: '#f4faf7',
              minHeight: '100vh',
              fontFamily: 'sans-serif'
            }}
          >
            <header
              style={{
                display: 'flex',
                height: 64,
                alignItems: 'center',
                gap: 40,
                overflow: 'auto',
                background: 'white'
              }}
            >
              <strong>1flowbase</strong>
              {topbar.map((node) => (
                <TopbarNavigationItemLabel
                  key={node.id}
                  workspaceId="fixture"
                  node={node}
                  siblings={topbar}
                >
                  <a href="#" style={{ color: '#192923' }}>
                    {node.title}
                  </a>
                </TopbarNavigationItemLabel>
              ))}
            </header>
            <section style={{ display: 'flex', gap: 24, marginTop: 24 }}>
              <aside style={{ width: 280, height: 560, background: 'white' }}>
                <FrontStagePageTreeSidebar
                  pageTree={tree}
                  selectedPageId="usage"
                  canEdit
                  isOperationPending={false}
                  onAddGroup={noop}
                  onAddPage={noop}
                  onAddPageInGroup={noop}
                  onRenameNode={noop}
                  onUpdateNodeMetadata={noop}
                  onEditNodeTooltip={noop}
                  onMoveNode={noop}
                  onDeleteNode={noop}
                  onSelectPage={noop}
                  onMoveNodeToPosition={(source, target, position) => {
                    setTree((current) =>
                      moveNodeToTreePosition(current, source, target, position)
                    );
                    setSaves((current) => current + 1);
                  }}
                />
              </aside>
              <article style={{ background: 'white', padding: 24, flex: 1 }}>
                <h1>模型使用报表</h1>
                <div
                  data-testid="navigation-drag-ready"
                  data-save-count={saves}
                >
                  导航拖拽验证
                </div>
              </article>
            </section>
          </main>
        </App>
      </QueryClientProvider>
    </ConfigProvider>
  );
}
await loadApplicationI18nResources();
await appI18n.changeLanguage('zh_Hans');
createRoot(document.getElementById('root')!).render(<Fixture />);
