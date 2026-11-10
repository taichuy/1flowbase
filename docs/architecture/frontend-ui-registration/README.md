# Frontend UI Registration

前端贡献的执行授权和 UI 注册目标是两个独立维度。`runtime: native_react` 只声明执行载体，不表示它应出现在创建区块菜单。

| Manifest 声明 | 底座数据 | 消费入口 |
| --- | --- | --- |
| `ui_registration: component` | `frontend_block_catalog` + 插件所有的 `ui_component_records` | 组件管理、编辑器组件资源 |
| `ui_registration: code_template` | `frontend_block_catalog` 的官方模板投影；人工模板与修订另存 `ui_code_templates` | 代码模板、编辑器模板资源 |
| `block_registration: true` | `frontend_block_catalog.block_registration` | 区块管理、创建区块 |

`block_registration` 从宿主 0.6.3 起可用，默认为 `false`；需要直接放入页面的贡献必须显式声明 `true`。`ui_registration` 在组件与模板中二选一；`block_registration` 独立决定是否允许直接创建。需要同时提供组件和模板时，使用不同贡献编码，可共享 `code_modules`。挂载源码不产生任何额外注册目标。

```yaml
block_contributions:
  - contribution_code: Bazi
    ui_registration: component
    block_registration: false
    runtime: native_react
    entry: default
    code_template_file: ui/components/Bazi.tsx
    code_template_version: 0.1.19
    code_template_language: tsx
    # 挂载源码、模块/资源摘要、上下文与权限继续使用前端贡献契约。
```

上述片段展示注册与源码入口，完整声明还需 `code_modules`、上下文与权限。受管插件仍使用 `1flowbase.workspace.frontend-block-contribution` v1、`declarative_only` 执行绑定及宿主规定的挂载授权。

## 包内源码引用

Manifest 声明注册元数据和文件位置，组件、模板实现与类型声明维护在独立文件中。宿主 0.6.3 支持：

| 字段 | 文件内容 | 约束 |
| --- | --- | --- |
| `block_contributions[].code_template_file` | 组件挂载入口或完整可编辑模板 | `.jsx` / `.tsx` 与 `code_template_language` 一致；最多 256 KiB |
| `code_modules[].type_declarations_file` | 完整 TypeScript 类型声明 | `.d.ts`；最多 2 MiB；设置页模块也支持 |

这两个字段只表达文件引用，不改变 `ui_registration` / `block_registration`。`code_modules[].source` 仍是代码中的模块导入标识，不是文件路径；`assets[].path` 继续定位已声明摘要的浏览器资源。

文件路径相对插件包根目录，必须随包交付；不得使用绝对路径、`..`、反斜杠或通过符号链接逃出包根目录。底座在安装准备阶段读取 UTF-8 文件，拒绝缺失、目录、空文件、超限内容和双重源码声明。旧的内联 `code_template` / `type_declarations` 仍可读取，但同一项不能同时提供内联文本和对应文件引用。

安装事务保存解析后的源码与类型声明快照，不将本机路径交给前端，也不改写已签名 manifest。编辑器、组件目录和模板目录继续消费底座数据。日历插件将交付文件放在打包器已包含的 `ui/` 目录；生成器复制组合模板与公开类型文件，生成宿主绑定声明，YAML 只记录引用，不再内联实现。

底座校验 manifest 后，在安装/版本切换事务中写入目录并同步组件投影。组件使用持久化插件所有者与贡献编码识别，更新不按展示名匹配，也不覆盖人工记录。官方代码模板按声明从目录投影，无需重复插入人工模板表。

`GET /api/console/frontend-blocks` 返回当前授权的运行贡献；添加 `registration_target=block|component|code_template` 可按显式用途筛选。响应 `registration_targets` 由后端提供。创建入口和区块管理使用 `registration_target=block`；运行/导入仍使用完整授权目录。

用户复制产生的定义独立保存在 `custom_frontend_blocks`，注册目标仅为 `block`。源码和名称独立保存，运行仍依赖当前有效基础贡献的授权、版本与资源；删除定义不删除已保存到页面的源码实例。

日历插件当前契约：6 个 `component`、1 个 `code_template`，所有条目的 `block_registration: false`；不得自动投影到创建区块目录。
