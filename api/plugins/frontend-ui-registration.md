# 插件前端 UI 注册

[English](frontend-ui-registration.en.md)

`block_contributions[].ui_registration` 明确声明管理入口：

| 值 | 注册入口 | 行为 |
| --- | --- | --- |
| `code_template`（省略时默认） | 设置 → UI 管理 → 代码模板 | 提供可复制、编辑、发布的完整区块模板；不自动生成 UI 组件记录。 |
| `component` | 设置 → UI 管理 → 组件管理 | 自动生成插件拥有的只读 UI 组件；不出现在设置及工作区的代码模板列表。 |

两种注册互斥；底座不能仅凭存在 `code_template` 源码就同时注册两个入口。
`component` 必须使用 `native_react` 并提供非空 `code_template`、版本和 JSX/TSX 语言；
此源码仍用于区块挂载和组件插入，注册目标不改变运行时模块/资产加载契约。

```yaml
block_contributions:
  - contribution_code: Calendar
    title: 日历
    runtime: native_react
    entry: default
    ui_registration: component
    code_template: "import { Calendar } from '@taichuy/taichuy-calendar'; export default () => <Calendar />;"
    code_template_version: 0.1.5
    code_template_language: tsx
    # code_modules / context_contract / permissions 等仍需按 manifest 契约声明
```

插件只声明贡献，底座负责安装事务内注册、同发布者/贡献的幂等升级、只读归属和卸载清理。
现有手工模板和组件不被删除或改写。旧包省略字段仍是代码模板；要改为 UI 组件，升级插件版本并显式声明 `component`。
需要支持本字段的底座源码构建，不能将相同版本号的旧发布包视为兼容。

## 组件编码

插件组件由底座按 `<publisher_namespace>.<artifact_id>.<组件名>` 生成 `component_code`：

- 发布者与插件标识来自持久安装记录，不从 npm scope、展示名或前端推断。
- 组件名来自 `contribution_code`：拆开 PascalCase / camelCase（含缩写边界），非 ASCII 字母数字的连续分隔符变为 `-`，去掉首尾 `-`，统一小写。例如 `BirthInfo` → `birth-info`、`HTTPClient` → `http-client`。
- 版本号不进入编码，升级保留组件 UUID 和归属；组件名归一化后必须非空。
- 编码转换不是全局唯一性的证明；不同贡献或 owner 生成相同编码时拒绝并回滚整个事务，不覆盖、不自动追加哈希或编号。
- 存量 `plugin.<哈希>` 由正式 migration 原地改码；官方和自定义记录、组件源码及页面引用不改动。

以上文日历包（`publisher_namespace: 1flowbase`、`plugin_id: taichuy-calendar`）为例：

| contribution_code | component_code |
| --- | --- |
| Calendar | `1flowbase.taichuy-calendar.calendar` |
| Almanac | `1flowbase.taichuy-calendar.almanac` |
| BirthInfo | `1flowbase.taichuy-calendar.birth-info` |
| ZiWei | `1flowbase.taichuy-calendar.zi-wei` |
| Bazi | `1flowbase.taichuy-calendar.bazi` |
| CalendarPage | `1flowbase.taichuy-calendar.calendar-page` |
