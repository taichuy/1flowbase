---
memory_type: feedback
feedback_category: repository
topic: Frontstage JSX 区块设计态与编辑体验
summary: Frontstage 区块操作必须复用紧凑 hover 图标语言；设计画布只呈现运行效果，诊断留在编辑器运行区；上下文进入源码注释；接口连接器只负责把 OpenAPI 转成完整代码片段并插入光标，不建立用户可见的持久绑定状态；组件目录必须显式注册真实 import、来源、支持的 API 与限制，以表格提供插入和复制 API 文档，并支持后端分页查询供未来 MCP 复用。
keywords:
  - frontstage
  - JSX Studio
  - Monaco
  - capability connector
  - resizable drawer
  - hover actions
  - generated context comment
  - component catalog
  - component API contract
  - TSDoc
  - MCP discovery
  - runtime diagnostics
  - module bindings
match_when:
  - 修改 Frontstage 页面、Tab 或区块的设计态操作入口
  - 修改 JSX 区块代码编辑、配置、接口绑定或变量注入体验
created_at: 2026-07-17 23
updated_at: 2026-08-11 16
last_verified_at: 2026-08-11 16
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
  - web/app/src/shared/schema-ui
  - api/apps/api-server/src/routes/frontstage
---

# Frontstage JSX Studio 交互规则

## 规则

- 区块 hover 操作复用左侧页面树的微型图标尺寸、间距、显隐和选中语言；仅换成相同颜色但保留白色胶囊工具条不算统一。
- 配置图标和编辑图标进入同一个共享可拉伸 JSX Studio，只改变初始焦点；不要继续维护两个割裂的固定宽度抽屉。
- 接口连接器、可用变量、组件目录和结构化配置必须在 Monaco 中形成可见前言、类型或可插入代码，并与运行时允许能力同源；只读 `Descriptions` 不是配置能力。
- 设计模式只改变区块的选择框与操作入口，区块内容仍只呈现代码运行后的 UI；日志、能力调用和协议拒绝属于运行诊断，不得作为“无”字段堆在画布区块内。
- 自动生成的上下文必须作为 Monaco 源码中的注释出现，用户可以删除并保存；需要恢复或更新时提供显式重新注入，锚定区块唯一入口，不单独渲染只读上下文区块。
- 组件目录不得再从类型声明中只推断组件名；组件注册必须携带真实 import source、实现来源（例如受控 Ant Design facade 或 1flowbase 自定义组件）、支持的 props / children / action 或 event、明确限制和可运行示例。description 的职责是压缩呈现这份 API 规范，不是只写“适合做什么”的用途文案。
- 不为组件文档发明 `@1flowbase-component` 等私有注释协议；组件源码、Monaco 和复制给外部 AI 的内容使用标准 TypeScript 类型声明与 JSDoc / TSDoc，展示真实 import 和 JSX 示例。编辑器组件区使用“组件 / description / 操作”三列表格，操作至少包含插入代码和复制 API 文档。
- 组件说明、编辑器列表和未来 MCP 查询必须来自同一个后端组件目录；查询支持稳定分页。组件注册写入口与真实 runtime/module 安装绑定，不允许独立注册一条文档后宣称组件可用。
- 接口连接允许在当前光标处插入代码，但单次插入必须是源码可见、可编辑的完整单元：实体类型、参数类型、返回类型与命名函数；不得只插入一行 `ctx.data.query(...)` / `ctx.actions.invoke(...)`，也不得用不透明虚拟模块把接口细节全部藏掉。连接器不自动改写 `main`，`main` 只保留清晰的顶层编排；变量连接可插入对应的命名只读变量。
- 接口连接器的产品职责到“生成并插入代码片段”为止；不要要求用户先绑定、展示“已绑定接口”、提供取消绑定，或让 alias / schema digest 成为额外可管理对象。运行时需要的 operation identity、Schema freshness、权限校验与写授权应由生成函数和受控 runtime contract 吸收，不能泄漏成第二套编程状态。
- 单次接口插入的阅读单位必须是一个可直接调用的函数或 callable 变量，参数与返回结构服务于这个入口；不要把响应对象每一层机械展开为一组顶层 `interface`，让类型声明淹没调用方法。
- Frontstage 内部 HTTP/OpenAPI 接口的作者侧身份优先使用规范化 `method + path template`；不要把 `operationId`、`schemaDigest` 或 descriptor 对象泄漏到生成源码。绝对 URL、认证、权限、写授权和 Catalog 解析仍由受控 Host/Backend 吸收。
- 生成的 callable 函数 / 变量名也是作者源码，只是可重命名的本地别名；必须使用可读业务语义，不得从 `interface_id`、`operationId` 或数据定义 UUID 生成。真正的运行接口身份仍是 `method + path template`。
- JSX Studio 的树状资源编辑应复用 MCP 管理器的成熟语言：搜索位于树上方，节点 hover 显示上下文动作，拖拽负责排序/移动，创建动作进入完整表单而不是先在面板外选类型再生成匿名节点。
- 当前区块是子容器树的概念根节点，但不持久化为一个 `child_container`；界面只提供“新增子容器”，顶层子容器是当前区块的直接孩子，不再暴露“新增根容器”。
- 子容器展示类型除 Drawer、Modal、Inline 外还包括 Page；Page 仍是普通树节点，不额外禁止 Page 创建 Page。无限递归由有限数据、无环约束、惰性/有界查询和导航状态机控制，不用类型层级禁令削弱树表达能力。
- Studio 工作区需要多 Tab 时，Tab 必须对应真实可维护对象。子容器自身若没有源码真值，不得伪造空的 TSX 代码 Tab；可打开容器配置 Tab，并从中继续打开其挂载 Block 的真实代码 Tab。只有明确引入 container source contract 后，容器才可拥有代码 Tab。
- 子容器层级不能把全部节点 Schema、代码和子树塞进一条 Page Document 数据。每个子容器应是拥有独立 `block_id`、代码与 Schema 的真实区块记录，父子关系使用可查询的 ordered adjacency tree；Schema UI 查询树摘要，点击节点后按需加载该区块并打开真实代码 Tab。
- 子区块间的代码引用不属于树结构的可维护外键：节点注册让 AI/Schema UI 可查询子区块，代码按 `block_id` 查询或打开；删除不因静态“事件引用”被数据层阻止。提供叶删除与显式子树删除，残留源码引用在运行时返回 node-not-found 诊断。

## 原因

用户需要统一、紧凑的设计态交互，也需要开发者和 AI 打开编辑器即可知道可用能力。源码已经是接口调用的作者真值；再维护 Block `interfaces`、已绑定列表和源码调用三份状态，会产生漂移并把简单 codegen 变成编程配置。运行诊断混入画布同样会改变区块真实输出语义并制造视觉噪声。

## 适用场景

Frontstage 页面设计模式、JSX 区块工具条、Monaco 编辑器、区块配置、capability Catalog 和运行时绑定策略。
