# Workspace Design Routing

## Truth and Lookup

[DESIGN.md](../../../../DESIGN.md) 是页面设计真值。本文件只保存检索与应用方法，不复制 recipe、L1 模型、颜色或移动端参数。

| 当前问题 | DESIGN.md 章节 | 实现取证 |
| --- | --- | --- |
| 属于哪种视觉层 | §1 真值、§2 三种视觉表达层 | theme、app-shell、当前 feature |
| 页面主任务与辅助区域 | §5 页面构造、§8 Recipe | 当前 route 与 section 定义 |
| Drawer / Inspector / Modal 选择 | §5.3 详情模型、§7.6 浮层、§9 Editor UI | 同类对象入口、焦点与关闭行为 |
| 状态、类型与选中颜色 | §3 语义颜色、§9 Editor UI | token、状态 DTO、共享组件 |
| 控件与交互表达 | §6 组件选择、§7 组件规则 | shared/ui 与调用方 |
| 小屏布局与键盘 / 触摸 | §10 响应式与可访问性 | 受影响页面运行态 |
| 局部样式能否承接 | §11 样式边界 | token → 自有 wrapper → 显式 slot |

按标题定位当前章节，章节编号只帮助查找。先确定任务域、详情模型和状态语义，再处理视觉细节。

## Source Ownership

- 应用详情 section 查 `web/app/src/features/applications/lib/application-sections.tsx`。
- 设置 section 查 `web/app/src/features/settings/lib/settings-sections.tsx`。
- 核对当前定义及其消费者，不复制侧栏、路由或权限真值。
- 已批准流程与 DESIGN.md / 当前实现不一致时，指出具体差异和影响；已有授权覆盖的实现修正直接继续，需要改变用户结果或设计约束时回到需求对齐。

## Evidence

按实际变化验证任务结构、详情行为、状态语义或小屏路径。合法反例：创建 / 确认可以使用 DESIGN.md 允许的 Modal，不把它误判为新增第三种常规详情模型。验收方式见 [review-checklist](review-checklist.md)。
