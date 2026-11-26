import { lazy, Suspense, useState } from 'react';

import { Link } from '@tanstack/react-router';
import { Button, Menu } from 'antd';
import type { MenuProps, TabsProps } from 'antd';

import type { SectionNavItem } from './SectionPageLayout';
import { i18nText } from '../../i18n/text';

const LazyAntdTabs = lazy(() => import('antd/es/tabs'));
const LazyAntdDrawer = lazy(() => import('antd/es/drawer'));

interface SectionSidebarNavProps {
  navItems: SectionNavItem[];
  activeKey: string;
  compactMode: boolean;
  compactVariant: 'tabs' | 'drawer';
}

function createMenuItems(navItems: SectionNavItem[]): MenuProps['items'] {
  return navItems.map((item) => ({
    key: item.key,
    icon: item.icon,
    label: (
      <Link className="section-page-layout__nav-link" to={item.to}>
        {item.label}
      </Link>
    )
  }));
}

function createTabItems(navItems: SectionNavItem[]): TabsProps['items'] {
  return navItems.map((item) => ({
    key: item.key,
    label: (
      <Link className="section-page-layout__nav-link" to={item.to}>
        {item.label}
      </Link>
    )
  }));
}

export function SectionSidebarNav({
  navItems,
  activeKey,
  compactMode,
  compactVariant
}: SectionSidebarNavProps) {
  const [drawerOpen, setDrawerOpen] = useState(false);

  if (navItems.length === 0) {
    return null;
  }

  const menuItems = createMenuItems(navItems);

  if (!compactMode) {
    return (
      <nav aria-label="Section navigation" className="section-page-layout__nav">
        <Menu mode="inline" selectedKeys={[activeKey]} items={menuItems} />
      </nav>
    );
  }

  if (compactVariant === 'tabs') {
    return (
      <nav aria-label="Section navigation" className="section-page-layout__nav">
        <div className="section-page-layout__mobile-tabs">
          <Suspense
            fallback={navItems.map((item) => (
              <Link
                key={item.key}
                className="section-page-layout__nav-link"
                to={item.to}
              >
                {item.label}
              </Link>
            ))}
          >
            <LazyAntdTabs
              activeKey={activeKey}
              items={createTabItems(navItems)}
            />
          </Suspense>
        </div>
      </nav>
    );
  }

  return (
    <div className="section-page-layout__drawer-trigger">
      <Button type="default" onClick={() => setDrawerOpen(true)}>
        {i18nText('sharedUi', 'auto.more_sections')}
      </Button>
      {drawerOpen ? (
        <Suspense fallback={null}>
          <LazyAntdDrawer
            title={i18nText('sharedUi', 'auto.more_sections')}
            placement="left"
            open={drawerOpen}
            onClose={() => setDrawerOpen(false)}
          >
            <nav
              aria-label="Section navigation"
              className="section-page-layout__nav"
            >
              <Menu mode="inline" selectedKeys={[activeKey]} items={menuItems} />
            </nav>
          </LazyAntdDrawer>
        </Suspense>
      ) : null}
    </div>
  );
}
