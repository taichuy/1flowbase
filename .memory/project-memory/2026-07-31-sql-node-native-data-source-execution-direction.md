---
memory_type: project
topic: SQL 节点原生数据源执行方向
summary: 单一 SQL 节点已落地：SQL 作为不透明文本原样执行，复用 SQLx/PgPool 与 Monaco；模板变量补全必须按部分输入过滤、排序并整段替换为 canonical token。
keywords:
  - sql-node
  - data-source
  - native-sql
  - runtime-extension
  - postgresql
match_when:
  - 设计或实现 AgentFlow SQL 节点
  - 扩展 data_source RuntimeExtension 查询协议
  - 判断 SQL 是否应由平台解析、转换或限制
created_at: 2026-07-31 10
updated_at: 2026-07-31 22
last_verified_at: 2026-07-31 22
decision_policy: verify_before_decision
scope:
  - api/crates/plugin-framework
  - api/crates/control-plane
  - api/crates/orchestration-runtime
  - api/apps/plugin-runner
  - api/apps/api-server
  - web/packages/flow-schema
  - web/app/src/features/applications
---

# SQL 节点原生数据源执行方向

## 时间

`2026-07-31 10`

## 谁在做什么

用户已确认 1flowbase 增加一个通用 SQL 节点。节点选择数据源实例并提交用户编写的 SQL；Host 将 SQL 作为不透明文本交给 Core PostgreSQL adapter 或声明原生 SQL 能力的 data-source RuntimeExtension，数据源执行后由平台回传结果或源端错误。

## 为什么这样做

统一节点只需要统一调用与结果 contract，不需要统一数据库方言。SQL 方言、驱动和执行语义由数据源实现拥有；平台不解析、翻译、改写或按 SELECT / DML / DDL 分类限制用户 SQL。

## 为什么要做

当前主数据源是 Core PostgreSQL，未来还会接入第三方数据源。该边界既允许 PostgreSQL 执行任意原生 SQL，也使以后支持原生 SQL 的数据源只需自声明能力并实现相同执行 contract，无需为每种数据库新增节点。

## 截止日期

未指定。当前为已确认架构方向；进入实现前按 Single Issue 固定 AC 与验证边界。

## 决策背后动机

本期优先建立完整执行能力。SQL 导致的数据变化和业务后果由用户承担；权限治理、只读模式、语句白名单和 SQL 风险管理不在本期。平台仍需如实返回行集、完成结果或数据库 / 传输错误，不能吞错或伪造成功。返回 contract 使用有序 `RowBatch | Completion`；`native_status` 可选，不要求 SQLx 伪造 command tag，零行原始查询允许只有 `Completion`。

## 成熟方案复审结论

- Core PostgreSQL Adapter 复用 SQLx 0.8.6 的 `raw_sql(sql).fetch_many()` 和现有共享 `PgPool`，不创建第二连接池，不引入 `tokio-postgres` 第二驱动。
- SQLx 不公开独立 `RowDescription` 或原始 `CommandComplete.tag`；结果 contract 必须服从该能力边界。
- V1 只有 `sql: string`，不支持 bind parameters、prepared statement、SQL parser、formatter 或方言转换。
- 前端复用已安装的 Monaco 并设置 `language="sql"`，不增加编辑器依赖。
- SQL 下发后连接中断返回 `outcome_unknown`；V1 不做隐式重试。
- 外部 data-source RuntimeExtension 当前为 `process_per_call + stdio_json`，不能跨调用维持连接池；持久 worker 是未来独立方向。
- 实现前必须先用 isolated PostgreSQL fixture 验证零行、多语句事件顺序和未知类型 raw value fallback。

## 实施形态

保持 Single Issue、不创建 Issue Tree、不使用并行开发 subagent。按 SQLx feasibility fixture → canonical contract/Host port → plugin-runner → Core PostgreSQL Adapter → Host/options → orchestration runtime → flow schema/Monaco UI → 集中 QA 串行完成。

## 实施结果

`2026-07-31 12` 已通过独立 worktree 分支 `codex/issue-1512-sql-node` 完成，并以 commit `b5170a197` fast-forward 合并回 `beta`。协议、PostgreSQL adapter、RuntimeExtension、编译/preview/full runtime、options API、OpenAPI/operation inventory、SQL 节点 schema/Inspector/Monaco 均已落地；主工作树原有未提交修改未被覆盖。

集中验证已覆盖 native SQL contract、SQLx 多语句顺序与类型 fallback、源端错误、plugin-runner opaque SQL、compiler/preview/full runtime、options route、route assembly、OpenAPI、前端 contract/validation/API client；合并后再次通过 PostgreSQL native SQL 2 tests 与前端 4 files / 86 tests。i18n hygiene 为 0 error，既有 211 warnings。运行态页面启动受开发数据库既有 migration checksum 不一致阻断，页面交互由用户在合并后人工验证。

## SQL 模板变量补全追加决策

`2026-07-31 22` 用户确认 SQL Monaco 的变量补全必须与模板文本编辑器保持同一查询语义：输入 `{sy` 时解析 `sy` 为部分查询，只展示真实匹配项并让 canonical selector path 前缀优先；选择后必须把触发符、查询文本和 Monaco 自动补出的右括号整段替换为 `{{source.path}}`，不得残留原查询或多余括号。单个 `{`、`{x}` 和 `Ctrl/Cmd + Space` 继续提供全量变量入口，候选标签保持完整的 `节点名/变量名` 或 `sys.variable`。

本轮实现将该规则收敛在 SQL `CodeSourceField` completion provider 内，不改变变量可见性、后端 contract、SQL 保存文本或运行时渲染。定向测试已覆盖无关候选过滤、`sys.*` 排序、完整标签和自动右括号替换范围；真实 Monaco 浏览器 smoke 受当前 `dev` Vite 8 依赖预优化长期 pending 阻断，未据此下浏览器通过结论。

## 结果转换声明与 fallback

- 数据源插件 / Adapter 声明并实现 `native_sql/v1`；Host 将有效能力投影到 ready 实例，实例不自行定义 native type 转换语义。
- 未声明 `native_sql/v1` 时返回 capability error，SQL 不下发。
- 已声明协议但遇到未知 native type 时返回 `logical_type: native`，优先使用 text，无法稳定文本化时使用 Base64 binary。
- text / binary 均无法取得时返回 `unsupported_result_type`；插件响应不符合 contract 时返回 `invalid_native_sql_result_contract`。
- 禁止退化成无 schema 的任意 JSON，Core `main` PostgreSQL Adapter 视为 Host 内建的 `native_sql/v1` 实现。

## 关联文档

- https://github.com/taichuy/1flowbase/issues/1512
- `api/plugins/README.md`
- `api/crates/plugin-framework/src/data_source_contract/mod.rs`
- `api/crates/control-plane/src/data_source/mod.rs`
- `api/crates/orchestration-runtime/src/execution_engine.rs`
