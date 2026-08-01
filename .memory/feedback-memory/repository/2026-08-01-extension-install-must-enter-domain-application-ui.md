---
memory_type: feedback
feedback_category: repository
topic: 扩展安装必须进入对应领域应用流程并复用已有组件
summary: 扩展中心完成产物安装后，不能用通用确认弹窗冒充领域生效；需要按后端返回的类型化下一步动作进入 Agent Flow、MCP、模型供应商等已有预览、导入或配置组件。
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
updated_at: 2026-08-01 21
last_verified_at: 2026-08-01 21
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
- 没有独立应用阶段的宿主型扩展可以在落盘后结束，但该结论必须由后端 application state 明确表达，不能靠前端静默省略。

## 原因

通用 `Modal.confirm` 只能确认供应链安装，无法展示领域冲突、依赖、导入对象、workspace 影响和后续配置。重复编写简化弹窗还会让扩展中心与原领域设置页面逐渐形成两套交互和状态判断。

## 停止条件

- 新流程需要前端按 `category` 猜测领域状态或拼装领域写请求。
- 为扩展中心复制一套已有预览 / 导入 UI，而不是抽取或直接复用领域组件。
