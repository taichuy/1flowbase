---
memory_type: feedback
feedback_category: interaction
topic: 后台事件页必须对齐共享表格页面和URL状态
summary: 用户纠正事件页应参照/settings/applications完整共享表格布局，tab有不同URL，并删除独立大标题空白面板。
created_at: "2026-11-06 08"
updated_at: "2026-11-06 09"
decision_policy: direct_reference
status: active
---

# 规则

- 用户要求后台共享表格时，应参照/settings/applications的整体组合：DataTableLayout、DataTable工具栏、列设置与完整分页区（总条数、页码、每页条数选择），不能只引入底层DataTable就宣称已对齐。
- 不给设置页额外添加单独大标题卡片；此轮事件中心的标题面板明确要求移除。
- 不同tab用不同URL表示，直接打开、刷新、前进/后退均从URL恢复选择。
- 共享表格行操作默认直接显示前两个按钮，从第三个起收纳进“更多”；所有操作和“更多”均通过普通单击触发，避免唯一的查看操作仍需先展开菜单。

- 事件行操作采用简短文案“详情”，不用“查看详情”（用户明确纠正）。

# 原因

此前事件页把按钮放在表格外，并加了占据大块空白的标题面板；tab只用React本地状态，既不符合参照页面也不能分享/恢复视图。

# 适用场景

事件中心和后续后台表格页面调整。数据契约、权限和事件业务语义仍以后端为准；此反馈不授权额外新增业务筛选或后台改造。
