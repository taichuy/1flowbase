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
    code_template: "import { Calendar } from '@example/calendar'; export default () => <Calendar />;"
    code_template_version: 1.0.0
    code_template_language: tsx
    # code_modules / context_contract / permissions 等仍需按 manifest 契约声明
```

插件只声明贡献，底座负责安装事务内注册、同发布者/贡献的幂等升级、只读归属和卸载清理。
现有手工模板和组件不被删除或改写。旧包省略字段仍是代码模板；要改为 UI 组件，升级插件版本并显式声明 `component`。
需要支持本字段的底座源码构建，不能将相同版本号的旧发布包视为兼容。
