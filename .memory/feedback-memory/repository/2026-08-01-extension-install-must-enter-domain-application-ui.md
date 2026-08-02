---
memory_type: feedback
feedback_category: repository
topic: 扩展安装必须进入对应领域应用流程并复用已有组件
summary: 扩展中心完成产物安装后，不能用通用确认弹窗冒充领域生效；需要按后端返回的类型化下一步动作进入 Agent Flow、MCP、多语言、模型供应商等领域已有的完整预览、导入、激活或配置流程，不能只复用末端展示弹窗。
keywords:
  - extension-center
  - application-handler
  - agent-flow
  - mcp
  - domain-ui
  - preview-import
match_when:
  - 扩展中心新增或修改安装、更新交互
  - 安装产物需要导入 workspace、激活目录或创建领域对象
  - 准备在扩展中心新写简化确认弹窗
created_at: 2026-08-01 21
updated_at: 2026-08-02 14
last_verified_at: 2026-08-02 14
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/pages/settings-page/SettingsExtensionCenterSection.tsx
  - web/app/src/features/applications
  - web/app/src/features/settings/components/mcp-management
  - api/apps/api-server/src/routes/plugins_and_models/plugins/extension_center
---

# 扩展安装必须进入对应领域应用流程并复用已有组件

## 规则

- `api/plugins` 中产物已安装与领域对象已导入、已激活或已配置是两个不同状态；前端不得把安装成功提示当作领域生效。
- 后端安装结果应返回类型化的下一步领域动作及稳定标识，前端只消费该 contract，不根据分类自行推断。
- `agent-flow` 应复用 Applications 已有模板预览与导入组件；MCP 应复用 MCP Management 已有 Bundle 预览与导入组件；模型供应商应复用供应商目录的安装、更新与配置交互。
- 这里的“复用”包含领域流程 controller、冲突与风险确认语义、写操作、结果反馈和 query 刷新；只抽取末端 Modal 展示组件，但在扩展中心另写一套流程编排，不算完成复用。
- 多语言目录的版本预览与激活流程由 `/settings/i18n` 领域页面拥有，扩展中心调用同一流程；MCP 与多语言分类还应提供到对应管理页面的明确链接，让安装后的持续管理有正式入口。
- 领域应用状态不能只看新流程写入的 receipt；历史领域对象已经存在时必须先做语义 reconciliation。若 Bundle 全部因为同 ID 而跳过，UI 必须明确显示“不会产生任何变更”，不能继续用“导入成功 / 已应用”掩盖 no-op。
- 没有独立应用阶段的宿主型扩展可以在落盘后结束，但该结论必须由后端 application state 明确表达，不能靠前端静默省略。

## Agent Flow 模板边界

- `agent-flow` 是远程模板发布版本与本地模板 JSON 库，不使用“安装”术语；本地没有模板时“导入”先下载、验签并保存模板，再从本地 JSON 创建新应用。
- 发布侧只优化现有“导出应用”入口，不新增“导出发布版本”等平行入口；发布版本固定存储在数据库中，重复导出不改变版本，只有形成一次新发布时才原子递增一次；导出记录当前系统版本，仓库流水线负责 checksum、签名、发布产物与 catalog 的一致生成。
- 本地模板库不建立数据库版本表；模板 JSON、验签回执与当前版本指针落在 `storage` 下的 Agent Flow 专用目录，文件系统是本地模板库存的唯一真值。
- 本地已有模板时，“导入”只读取当前选中的本地版本创建新应用，不重复请求远程；“同步”只拉取远程模板新版本并更新本地模板库，绝不修改任何已创建应用。
- 模板详情必须能查看远程与本地历史版本；本地版本可设为当前导入版本、用于创建新应用或删除。切换、同步、删除本地模板版本都不得影响已经导入的应用。
- 应用可以记录来源模板 ID、发布版本与 checksum 作为 provenance，但该记录不能建立自动更新关系。

## 原因

通用 `Modal.confirm` 只能确认供应链安装，无法展示领域冲突、依赖、导入对象、workspace 影响和后续配置。重复编写简化弹窗还会让扩展中心与原领域设置页面逐渐形成两套交互和状态判断。

仅凭 import receipt 判断 `applied` 会把迁移前已经存在的领域数据误判为未应用；反过来，无条件为全量 `skipped` 的导入写 receipt 又会把零写入误判为已应用。状态必须同时表达实际 effect plan 与 reconciliation 结果。

## 停止条件

- 新流程需要前端按 `category` 猜测领域状态或拼装领域写请求。
- 为扩展中心复制一套已有预览 / 导入 UI，而不是抽取或直接复用领域组件。
- 扩展中心只共享展示 Modal，却继续独立持有领域预览、确认、写入和刷新状态。
- 预览结果已经证明全部项目冲突或跳过，仍允许执行一个没有业务效果、却可能写入“已应用”标记的动作。
