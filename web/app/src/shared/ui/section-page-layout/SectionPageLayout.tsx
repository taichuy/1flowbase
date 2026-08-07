import type { ReactNode } from 'react';

import { Grid, Layout, Typography } from 'antd';

import { SectionSidebarNav } from './SectionSidebarNav';
import './section-page-layout.css';

export interface SectionNavItem {
  key: string;
  label: string;
  to: string;
  icon?: ReactNode;
  group?: string;
  visible?: boolean;
}

export interface SectionPageLayoutProps {
  pageTitle?: ReactNode;
  pageDescription?: ReactNode;
  navItems: SectionNavItem[];
  activeKey: string;
  children: ReactNode;
  sidebarContent?: ReactNode;
  sidebarFooter?: ReactNode;
  emptyState?: ReactNode;
  contentWidth?: 'wide' | 'narrow' | 'full';
  heightMode?: 'natural' | 'viewport';
  hideCompactNav?: boolean;
}

export function SectionPageLayout({
  pageTitle,
  navItems,
  activeKey,
  children,
  sidebarContent,
  sidebarFooter,
  emptyState,
  contentWidth = 'wide',
  heightMode = 'natural',
  hideCompactNav = false
}: SectionPageLayoutProps) {
  const screens = Grid.useBreakpoint();
  const visibleItems = navItems.filter((item) => item.visible !== false);
  const compactMode = !screens.lg;
  const compactVariant = visibleItems.length <= 5 ? 'tabs' : 'drawer';
  const hasSidebarContent = Boolean(sidebarContent);
  const hasSidebar = visibleItems.length > 0 || hasSidebarContent;
  const layoutClassName = [
    'section-page-layout',
    `section-page-layout--${contentWidth}`,
    heightMode === 'viewport' ? 'section-page-layout--viewport' : null
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <Layout className={layoutClassName} data-testid="section-page-layout">
      {!hasSidebar ? (
        <Layout.Content className="section-page-layout__content">
          {emptyState ?? children}
        </Layout.Content>
      ) : (
        <Layout className="section-page-layout__shell">
          {!compactMode ? (
            <Layout.Sider
              className="section-page-layout__rail"
              theme="light"
              width={180}
            >
              {pageTitle ? (
                <Typography.Title
                  className="section-page-layout__title"
                  level={4}
                >
                  {pageTitle}
                </Typography.Title>
              ) : null}
              <SectionSidebarNav
                navItems={visibleItems}
                activeKey={activeKey}
                compactMode={false}
                compactVariant={compactVariant}
              />
              {hasSidebarContent ? (
                <div className="section-page-layout__sidebar-content">
                  {sidebarContent}
                </div>
              ) : null}
              {sidebarFooter ? (
                <div className="section-page-layout__footer">{sidebarFooter}</div>
              ) : null}
            </Layout.Sider>
          ) : null}

          <Layout.Content className="section-page-layout__content">
            {compactMode ? (
              <>
                {pageTitle ? (
                  <Typography.Title level={4} style={{ marginTop: 0 }}>
                    {pageTitle}
                  </Typography.Title>
                ) : null}
                {hasSidebarContent ? (
                  <div className="section-page-layout__sidebar-content">
                    {sidebarContent}
                  </div>
                ) : !hideCompactNav ? (
                  <SectionSidebarNav
                    navItems={visibleItems}
                    activeKey={activeKey}
                    compactMode
                    compactVariant={compactVariant}
                  />
                ) : null}
              </>
            ) : null}
            {children}
          </Layout.Content>
        </Layout>
      )}
    </Layout>
  );
}
