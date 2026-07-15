import fs from 'node:fs';
import path from 'node:path';
import type { ReactElement } from 'react';

import { render, screen } from '@testing-library/react';
import {
  Outlet,
  RouterProvider,
  createRootRoute,
  createRoute,
  createRouter
} from '@tanstack/react-router';
import { Grid } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { SectionPageLayout, type SectionNavItem } from '../SectionPageLayout';

const useBreakpointSpy = vi.spyOn(Grid, 'useBreakpoint');

const navItems: SectionNavItem[] = [
  { key: 'profile', label: '个人信息', to: '/me/profile' },
  { key: 'security', label: '安全设置', to: '/me/security' },
  { key: 'notifications', label: '通知偏好', to: '/me/notifications' },
  { key: 'devices', label: '登录设备', to: '/me/devices' },
  { key: 'tokens', label: '访问令牌', to: '/me/tokens' }
];

function renderInRouter(layout: ReactElement, pathname = '/') {
  window.history.pushState({}, '', pathname);

  const rootRoute = createRootRoute({
    component: () => <Outlet />
  });
  const pageRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    component: () => layout
  });
  const router = createRouter({
    routeTree: rootRoute.addChildren([pageRoute])
  });

  return render(<RouterProvider router={router} />);
}

describe('SectionPageLayout', () => {
  beforeEach(() => {
    useBreakpointSpy.mockReturnValue({
      xs: true,
      sm: true,
      md: true,
      lg: true,
      xl: false,
      xxl: false
    });
  });

  test('renders desktop rail navigation and sidebar footer', async () => {
    const view = renderInRouter(
      <SectionPageLayout
        pageTitle="个人资料"
        pageDescription="管理个人资料与安全设置"
        navItems={navItems.slice(0, 2)}
        activeKey="profile"
        sidebarFooter={<button type="button">退出登录</button>}
      >
        <section>个人资料内容</section>
      </SectionPageLayout>
    );

    expect(await screen.findByRole('navigation')).toBeInTheDocument();
    expect(screen.getByText('个人资料内容')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '退出登录' })
    ).toBeInTheDocument();
    expect(screen.getByTestId('section-page-layout')).toHaveClass('ant-layout');
    expect(
      view.container.querySelector('.section-page-layout__rail')
    ).toHaveClass('ant-layout-sider');
    expect(
      view.container.querySelector('.section-page-layout__content')
    ).toHaveClass('ant-layout-content');
  });

  test('uses wide content width by default for management pages', async () => {
    const view = renderInRouter(
      <SectionPageLayout
        pageTitle="设置"
        navItems={navItems.slice(0, 2)}
        activeKey="profile"
      >
        <section>宽布局内容</section>
      </SectionPageLayout>
    );

    expect(await screen.findByText('宽布局内容')).toBeInTheDocument();
    expect(screen.getByTestId('section-page-layout')).toHaveClass(
      'section-page-layout',
      'section-page-layout--wide'
    );
    view.unmount();
  });

  test('supports explicit narrow mode for form pages', async () => {
    const view = renderInRouter(
      <SectionPageLayout
        pageTitle="个人资料"
        navItems={navItems.slice(0, 2)}
        activeKey="profile"
        contentWidth="narrow"
      >
        <section>窄布局内容</section>
      </SectionPageLayout>
    );

    expect(await screen.findByText('窄布局内容')).toBeInTheDocument();
    expect(screen.getByTestId('section-page-layout')).toHaveClass(
      'section-page-layout--narrow'
    );
    view.unmount();
  });

  test('supports viewport height without changing the content width variant', async () => {
    renderInRouter(
      <SectionPageLayout
        pageTitle="设置"
        navItems={navItems.slice(0, 2)}
        activeKey="profile"
        contentWidth="wide"
        heightMode="viewport"
      >
        <section>固定高度内容</section>
      </SectionPageLayout>
    );

    expect(await screen.findByText('固定高度内容')).toBeInTheDocument();
    expect(screen.getByTestId('section-page-layout')).toHaveClass(
      'section-page-layout--wide',
      'section-page-layout--viewport'
    );
    expect(screen.getByTestId('section-page-layout')).not.toHaveClass(
      'section-page-layout--full'
    );
  });

  test('does not couple the content offset to the old centered 1200px shell width', () => {
    const sectionLayoutCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../section-page-layout.css'),
      'utf8'
    );

    expect(sectionLayoutCss).not.toContain(
      'margin-left: calc(240px - max(0px, (100vw - min(1200px, calc(100vw - 48px))) / 2));'
    );
    expect(sectionLayoutCss).toContain('gap: 16px;');
    expect(sectionLayoutCss).toContain('position: sticky;');
    expect(sectionLayoutCss).not.toContain('position: fixed;');
  });

  test('does not force the desktop rail to viewport height on short pages', () => {
    const sectionLayoutCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../section-page-layout.css'),
      'utf8'
    );
    const desktopRailBlock = sectionLayoutCss.match(
      /\.section-page-layout__rail\s*\{[\s\S]*?\n\}/
    )?.[0];

    expect(desktopRailBlock).toContain('.section-page-layout__rail');
    expect(desktopRailBlock).not.toContain('min-height: calc(100vh - 56px);');
  });

  test('does not use negative margins to align section layout with the app shell', () => {
    const sectionLayoutCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../section-page-layout.css'),
      'utf8'
    );

    expect(sectionLayoutCss).not.toContain('margin: -28px');
    expect(sectionLayoutCss).not.toContain('margin: -20px');
    expect(sectionLayoutCss).toContain('--section-page-header-height: 56px;');
  });

  test('keeps the desktop section rail width compact and synchronized', () => {
    const sectionLayoutCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../section-page-layout.css'),
      'utf8'
    );
    const sectionLayoutSource = fs.readFileSync(
      path.resolve(import.meta.dirname, '../SectionPageLayout.tsx'),
      'utf8'
    );

    expect(sectionLayoutCss).toContain('--section-page-rail-width: 180px;');
    expect(sectionLayoutSource).toContain('width={180}');
  });

  test('bounds full section layouts to the app shell viewport', () => {
    const sectionLayoutCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../section-page-layout.css'),
      'utf8'
    );
    const fullLayoutBlock = sectionLayoutCss.match(
      /\.section-page-layout--full\s*\{[\s\S]*?\n\}/
    )?.[0];
    const fullShellBlock = sectionLayoutCss.match(
      /\.section-page-layout--full \.section-page-layout__shell\s*\{[\s\S]*?\n\}/
    )?.[0];
    const fullContentBlock = sectionLayoutCss.match(
      /\.section-page-layout--full \.section-page-layout__content\s*\{[\s\S]*?\n\}/
    )?.[0];
    const fullRailBlock = sectionLayoutCss.match(
      /\.section-page-layout--full \.section-page-layout__rail\s*\{[\s\S]*?\n\}/
    )?.[0];

    expect(fullLayoutBlock).toContain(
      'height: calc(100vh - var(--section-page-header-height));'
    );
    expect(fullLayoutBlock).toContain('overflow: hidden;');
    expect(fullShellBlock).toContain('height: 100%;');
    expect(fullShellBlock).toContain('min-height: 0;');
    expect(fullRailBlock).toContain('top: 0;');
    expect(fullContentBlock).toContain('min-height: 0;');
    expect(fullContentBlock).toContain('overflow: hidden;');
  });

  test('bounds explicit viewport section layouts while preserving normal spacing', () => {
    const sectionLayoutCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../section-page-layout.css'),
      'utf8'
    );
    const viewportLayoutBlock = sectionLayoutCss.match(
      /\.section-page-layout--viewport\s*\{[\s\S]*?\n\}/
    )?.[0];
    const viewportShellBlock = sectionLayoutCss.match(
      /\.section-page-layout--viewport \.section-page-layout__shell\s*\{[\s\S]*?\n\}/
    )?.[0];
    const viewportContentBlock = sectionLayoutCss.match(
      /\.section-page-layout--viewport \.section-page-layout__content\s*\{[\s\S]*?\n\}/
    )?.[0];
    const viewportRailBlock = sectionLayoutCss.match(
      /\.section-page-layout--viewport \.section-page-layout__rail\s*\{[\s\S]*?\n\}/
    )?.[0];

    expect(viewportLayoutBlock).toContain(
      'height: calc(100vh - var(--section-page-header-height));'
    );
    expect(viewportLayoutBlock).toContain('overflow: hidden;');
    expect(viewportShellBlock).toContain('height: 100%;');
    expect(viewportShellBlock).toContain('min-height: 0;');
    expect(viewportContentBlock).toContain('height: 100%;');
    expect(viewportContentBlock).toContain('min-height: 0;');
    expect(viewportContentBlock).toContain('overflow: hidden;');
    expect(viewportContentBlock).toContain('box-sizing: border-box;');
    expect(viewportContentBlock).toContain(
      'padding-inline-end: var(--section-page-viewport-inline-end-gap, 16px);'
    );
    expect(viewportContentBlock).toContain(
      'padding-bottom: var(--section-page-viewport-bottom-gap, 3px);'
    );
    expect(viewportRailBlock).toContain('position: static;');
  });

  test('lets full section layouts return to natural height on mobile', () => {
    const sectionLayoutCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../section-page-layout.css'),
      'utf8'
    );
    const mobileBlock = sectionLayoutCss.slice(
      sectionLayoutCss.indexOf('@media (max-width: 991px)')
    );

    expect(mobileBlock).toContain('.section-page-layout--full');
    expect(mobileBlock).toContain('height: auto;');
    expect(mobileBlock).toContain('overflow: visible;');
  });

  test('lets explicit viewport section layouts return to natural height on mobile', () => {
    const sectionLayoutCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../section-page-layout.css'),
      'utf8'
    );
    const mobileBlock = sectionLayoutCss.slice(
      sectionLayoutCss.indexOf('@media (max-width: 991px)')
    );

    expect(mobileBlock).toContain('.section-page-layout--viewport');
    expect(mobileBlock).toContain('height: auto;');
    expect(mobileBlock).toContain('overflow: visible;');
    expect(mobileBlock).toContain(
      'padding: var(--section-page-top-padding) 0;'
    );
  });

  test('renders empty state instead of broken navigation when navItems is empty', async () => {
    renderInRouter(
      <SectionPageLayout
        pageTitle="设置"
        navItems={[]}
        activeKey=""
        emptyState={<div>当前账号暂无可访问内容</div>}
      >
        <section>不会显示的内容</section>
      </SectionPageLayout>
    );

    expect(
      await screen.findByText('当前账号暂无可访问内容')
    ).toBeInTheDocument();
    expect(screen.queryByRole('navigation')).not.toBeInTheDocument();
    expect(screen.queryByText('不会显示的内容')).not.toBeInTheDocument();
  });

  test('renders custom sidebar content when navItems is empty', async () => {
    renderInRouter(
      <SectionPageLayout
        pageTitle="前台"
        navItems={[]}
        activeKey=""
        sidebarContent={<aside>页面树</aside>}
      >
        <section>前台画布</section>
      </SectionPageLayout>
    );

    expect(await screen.findByText('页面树')).toBeInTheDocument();
    expect(screen.getByText('前台画布')).toBeInTheDocument();
    expect(screen.queryByRole('navigation')).not.toBeInTheDocument();
  });

  test('switches to compact mobile navigation when breakpoint is below lg', async () => {
    useBreakpointSpy.mockReturnValue({
      xs: true,
      sm: true,
      md: true,
      lg: false,
      xl: false,
      xxl: false
    });

    const view = renderInRouter(
      <SectionPageLayout
        pageTitle="个人资料"
        navItems={navItems.slice(0, 4)}
        activeKey="profile"
      >
        <section>四个以内的移动导航</section>
      </SectionPageLayout>
    );

    expect(await screen.findByRole('tablist')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '更多分区' })
    ).not.toBeInTheDocument();

    view.unmount();

    renderInRouter(
      <SectionPageLayout
        pageTitle="个人资料"
        navItems={navItems}
        activeKey="profile"
      >
        <section>超过四个的移动导航</section>
      </SectionPageLayout>
    );

    expect(
      await screen.findByRole('button', { name: '更多分区' })
    ).toBeInTheDocument();
  });

  test('can hide compact navigation when another responsive selector owns it', async () => {
    useBreakpointSpy.mockReturnValue({
      xs: true,
      sm: true,
      md: true,
      lg: false,
      xl: false,
      xxl: false
    });

    renderInRouter(
      <SectionPageLayout navItems={navItems} activeKey="profile" hideCompactNav>
        <section>设置页内容</section>
      </SectionPageLayout>
    );

    expect(await screen.findByText('设置页内容')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '更多分区' })
    ).not.toBeInTheDocument();
    expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
  });
});
