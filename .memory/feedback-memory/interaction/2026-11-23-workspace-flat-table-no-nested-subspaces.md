---
memory_type: feedback
feedback_category: interaction
topic: 工作空间扁平主表、单行工具栏、共享ResizableDrawer、下拉搜索确认卡片、根空间管理员写死与多数据滚动验收
summary: 禁止表格嵌套子空间；工具栏单行左右对齐，行操作「管理员 / 配置 / 停用」禁单项更多；选人用下拉搜索+用户信息确认卡片；root空间管理员底座写死仅root不可改；插件抽屉共享底座ResizableDrawer；列表验收须造超一屏数据实测滚动。
keywords:
  - workspaces
  - multi-tenant
  - data-table
  - ResizableDrawer
  - user search card
  - root admin immutable
  - scroll verification
match_when:
  - 设计或修改工作空间 / 多租户插件、后台设置表格与共享抽屉、成员搜索选择器及列表滚动验收时
created_at: 2026-11-23 12
updated_at: 2026-11-25 11
last_verified_at: 2026-11-25 11
decision_policy: direct_reference
scope:
  - api/
  - web/
  - /home/taichuy/git/1flowbase-plugins-business/runtime-extensions/@taichuy/workspaces
---

# 工作空间扁平主表、两层权限边界与普通设置插件规则

## 时间

`2026-11-25 11`

## 规则

1. **禁止表格单元格嵌套子对象列表与行内操作**：主表严格 `1 行 = 1 个工作空间`，不暴露“租户 / 下属工作空间 / 归属已有租户 / 空间上限”。
2. **后台表格页禁止三行错位工具栏，统一单行左右对齐**：左侧放搜索、状态筛选与多选批量操作，右侧同行对齐放置 `刷新`、`列设置` 与主按钮 `新建工作空间`。
3. **行内操作文案精炼，禁止把仅剩 1 个的操作折叠进「更多」**：统一使用 `管理员`、`配置`、`停用` / `启用`；总操作数 `<= 3` 直接行内平铺。
4. **指定成员/管理员必须使用「下拉搜索 + 选中用户信息确认卡片」**：禁止裸文本框让用户盲输账号/邮箱/ID；通过远程搜索下拉框按姓名/账号/邮箱检索候选人，选中后在表单下方渲染用户信息确认卡片（姓名、`@账号`、邮箱、状态）供二次核对。
5. **`root` 系统根工作空间管理员底层写死仅限 `root` 用户，不可修改**：底座 `workspace_governance` 仓储层硬拦截对 `bootstrap_workspace_id` 的任何 `assign_workspace_admins` / `revoke_workspace_admin` 写操作，且列表投影根空间管理员固定只返回内置 `root` 账号；前端根空间行禁止勾选批量指定管理员、禁止增删管理员。
6. **插件优先共享底座封装组件（如 `ResizableDrawer`）**：底座 `shared/ui` 已有的通用交互容器（如可拖拽调宽抽屉 `ResizableDrawer`）应通过 `native-modules` 暴露给插件复用，禁止在插件内重复裸写低配版抽屉。
7. **权限分两层（先空间 → 后角色），不造内置角色**：`@taichuy/workspaces` 作为普通受管设置插件仅限在 `root` 空间的 `/settings/roles` 配置权限。
8. **列表页验收必须造超过一屏的测试数据验证真实滚动**：只有 2~3 行静态样本无法暴露 `viewport / flex / overflow` 裁切问题，验收时须构造多页/超一屏数据实测纵向滚动与分页。

## 原因

1. 裸文本框盲填用户标识极易输错且无反馈，下拉搜索配合信息卡片才能在提交前确认目标身份；
2. 根空间精细权限本就由 `/settings/roles` 承载，根空间管理员身份锁定为唯一内置 `root` 用户最清晰安全；
3. 复用底座 `ResizableDrawer` 保证插件页与原生设置页体验一致（拖拽调宽、无障碍、统一 footer 槽位）；
4. `SectionPageLayout` 采用 `100vh` 裁切布局，必须用超一屏真实数据才能验证内部滚动链条是否闭合。

## 适用场景

- `@taichuy/workspaces`、后台设置页 `DataTable` / `ResizableDrawer` 共享组件、用户选择器交互及列表滚动 QA 验收。
