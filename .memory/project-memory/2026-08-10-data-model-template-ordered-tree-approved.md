---
memory_type: project
topic: Data Model 模板注册与有序树表方案已批准
summary: 用户批准 DataSource × DataModelTemplate 平衡方案及 Root Issue #1647；内置 general/v1 与 ordered_tree/v1，树使用 parent_id + sibling_rank，模板创建后永久固定且不提供任何转换或迁移，只能删表重建。
keywords:
  - data-model-template
  - ordered-tree
  - adjacency-list
  - fractional-indexing
  - template-registry
  - issue-tree
match_when:
  - 执行或调整 Data Model 模板注册、有序树表或生成 Runtime API
  - 讨论普通表与树状表是否允许转换
  - 讨论外部数据源模板由 HostExtension 还是 RuntimeExtension 实现
created_at: 2026-08-10 00
updated_at: 2026-08-10 12
last_verified_at: 2026-08-10 12
decision_policy: verify_before_decision
scope:
  - api/crates/domain
  - api/crates/control-plane
  - api/crates/runtime-core
  - api/crates/plugin-framework
  - api/crates/storage-durable/postgres
  - api/apps/api-server
  - api/plugins
  - web/app/src/features/settings/components/data-models
---

# Data Model 模板注册与有序树表方案已批准

## 时间

`2026-08-10 00`

## 谁在做什么

用户已批准完整技术方向，并授权将其建立为 GitHub Root → Delivery Issue Tree。Root #1647 是计划、进度和最终用户验收的唯一在线真值；Delivery 为 #1648、#1649、#1650。

## 为什么这样做

DataSource 与 DataModelTemplate 是两个独立维度。Host/Core 统一拥有模板目录、兼容校验、权限、生命周期和 API/OpenAPI 注册；主 PostgreSQL 模板由 Core/可信 HostExtension 实现，外部数据源自己的模板和 operation handler 可由对应 RuntimeExtension 提供。

`ordered_tree/v1` 固定为 Ordered Rooted Forest：Adjacency List `parent_id` + Fractional Order Key `sibling_rank`。`depth/path/has_children` 只作查询投影；树结构写由后端专用 command、事务锁和循环校验拥有。

## 为什么要做

当前 Data Model 只生成六个默认字段和固定五个 CRUD API，既不能表达树结构，也没有让外部插件以统一字段/operation/权限/OpenAPI descriptor 注册新模板的边界。该方案在满足拖拽、祖先/后代、搜索、删除和插件扩展的同时，避免 Closure Table、Materialized Path 与通用图引擎的额外复杂度。

## 截止日期

无固定日期；按 #1647 生命周期执行与验收。

## 决策背后动机

- 模板创建后永久固定，不支持 `general ↔ ordered_tree` 或任何插件模板之间的转换、迁移、复制和数据搬运。
- 若需要更换模板，只能由用户确认删除原 Data Model/物理表后全新创建。
- 已有 Data Model 只回填 `general/v1` metadata，物理表、记录和现有 Runtime API 必须零变化。
- `ordered_tree/v1` 生成五个专门化 CRUD 和七个树 operation；客户端只提交 parent/anchor，不写 rank。
- descriptor、field、operation、permission、handler 和 OpenAPI 必须单一真值并 fail closed。

## 关联文档

- Root：https://github.com/taichuy/1flowbase/issues/1647
- Delivery：#1648 版本化模板注册与不可变创建
- Delivery：#1649 内置有序树表结构与专用 Runtime API
- Delivery：#1650 外部数据源模板扩展与 operation 注册

## 执行状态

- 最终实现已合入并推送 `dev`，verified candidate 为 `5aa15b2f66ba9da4de87cab77aceb4a39fe419ff`。
- Final Central QA 为 `QA_PASS`：AC-001～AC-014 全部 green；9 条 Cargo 定向命令、前端 183 files / 1021 tests、Rust static、i18n、contracts、api-client 和 desktop/mobile page-debug 均通过。
- external Core `general/v1` 已按 selected template write policy 投影系统字段；客户端无需提交 `id` 即可完成外部五 CRUD，普通 provider required 字段仍保持 create-required，客户端提交系统字段仍拒绝。
- 三种 frontend catalog refetch（incompatible/empty/error）由 per-test QueryClient 控制，pending 与 settled 均清空选择并 fail closed，`onMap=0`。
- #1648、#1649、#1650 已关闭；Root #1647 进入 `phase:user-acceptance`，等待用户最终验收。
- 最终证据：<https://github.com/taichuy/1flowbase/issues/1647#issuecomment-5234015201>；本地 QA 报告：`tmp/test-governance/qa-1647-central-rf3/report.md`。
- 用户验收修正 Delivery #1652 已完成：模板 Select 只显示业务名称，默认系统字段由 descriptor 投影并在下方只读表格展示；Data Model 新建/编辑统一复用 `SchemaFormDrawer`，外部 mapping selector 同样不展示内部 identity。
- #1652 Final QA 为 AC-015～AC-019 全绿、focused 42/42；已通过 merge commit `a079ae454df6a5582d53f50a8b226629eb377adc` 推送 `dev`，Root #1647 再次进入 `phase:user-acceptance`。
- 用户进一步确认创建表单顺序为模板、标题、Code、描述、开放 API，默认字段表固定在最底部且只显示 `Code / 类型 / 必填`。该修正已通过 merge commit `1fba5489ca624b6bf85c0575d9e765bfc214369a` 推送 `dev`；聚焦测试 40/40、TypeScript、定向 ESLint、真实页面顺序/表头及零 console error 均通过，Root #1647 保持 `phase:user-acceptance`。
- 用户进一步确认模板下拉应铺满表单宽度、开放 API 使用同行布局，并指出创建抽屉缺少拖拽宽度能力。已确认该入口本来就在使用 `SchemaFormDrawer`，缺口是调用方未启用 `ResizableDrawer`；修正通过 merge commit `b621179e3402dd5bc49a9556e84b818f856812de` 推送 `dev`，真实页面验证下拉与输入框等宽、同行中心对齐、宽度可由 560px 调至 600px 且原生 dragger 存在。
- 开放 API 的最终验收布局为：标题与 Switch 在第一行，提示说明使用统一 Form.Item 描述槽位放在下一行；桌面与移动端均已验证。该修正通过 merge commit `f664edf9f539de1b9ec1bc558707043482f7ce52` 推送 `dev`。
- Data Model 详情最终只展示业务字段“分类表类型”，其值由后端模板目录按持久化 identity 投影为 `template_summary`；`template_provider/template_code/template_version` 继续保留为内部契约但不向用户展示。该修正通过 merge commit `56a45e4f9` 推送 `dev`，定向前后端测试、静态门禁与真实 ordered-tree 页面验收均通过。
