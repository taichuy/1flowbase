---
memory_type: feedback
feedback_category: repository
topic: 前台设计模式的页面与标签页选择边界
summary: 前台设计模式中，页面、标签页与真实区块的选择边界必须与语义对象一致；通用 JSX 区块是系统内置、始终可用的基础执行容器，未来组件能力通过受控导入模块扩展，不做前端 fallback。
keywords:
  - frontstage
  - design mode
  - page tabs
  - selection boundary
  - drag handle
  - config popover
  - compact header
  - empty canvas
  - default JSX block
  - code editor
  - builtin JSX block
  - controlled imports
match_when:
  - 调整前台设计模式的页面、标签页或区块层级
  - 调整标签页选中态、拖拽入口或配置入口
  - 设计页面级与标签页级配置边界
created_at: 2026-07-17 17
updated_at: 2026-07-17 22
last_verified_at: 2026-07-17 22
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
---

# 前台设计模式的页面与标签页选择边界

## 时间

`2026-07-17 17`

## 规则

前台设计模式里，页面级交互归属于整个页面容器，不应把页面级配置框误绑到页面标题本身。

标签页是其下方全部区块内容的语义容器。切换或选中标签页时，设计态边框应覆盖该标签页对应的完整内容区域，包括区块画布与新增区块入口；标签文字只负责切换，不单独承担选择框。

标签页的排序和配置入口使用与左侧页面树一致的两个紧凑图标：独立拖拽手柄与配置 / 菜单图标；配置图标点击后打开弹出层，不把完整操作条常驻在标签文字内。

页面标题区的单行高度应约为当前 88px 的一半，标签栏紧跟标题分隔线；正常的“已同步”状态不常驻占据垂直布局，只在保存中或失败时显示状态。

空标签页不渲染带大面积背景和虚线边框的“空画布”，因为它会被误认为已创建的区块。设计态空内容只保留轻量空提示和一个创建入口；真实区块出现后才显示区块边框。

“创建区块”的主操作直接创建一个带有最小、可运行 JSX 示例的默认 JS 区块，不先弹出区块目录选择抽屉。选中真实区块后，工具条将“区块配置”和“编辑 JSX”作为两个相邻的紧凑图标，不把两者混在一个更多菜单中。

直接创建流程不能只在前端测试中 mock 官方 JS 区块 Catalog entry。用户验收前必须用真实 `GET /api/console/frontend-blocks` 证明运行库已注册官方默认贡献；注册缺失时应修复后端插件 / Catalog 投影链路，不得用前端 fallback 或手工插库伪造可用项。

通用 JSX 区块应由系统内置并保持稳定身份，它负责承载用户可编辑 JSX；`block-sdk`、`antd-facade` 和未来完成沙箱适配的组件是该区块的受控导入模块。不要把每个可导入组件再注册成一种区块类型，也不要开放未经适配的任意包导入。

## 原因

只框住标题或标签文字会把视觉选择边界和真实编辑对象拆开；同样，把空状态绘制成区块外观，也会让用户无法判断数据中是否真有区块。紧凑的层级间距、明确的对象边界和直达主操作可以让设计态更接近所见即所得。

## 适用场景

修改 `FrontStagePage`、`FrontstagePageTabs`、标签页内容布局、设计态选中框、标签页排序与配置弹出层时命中。
