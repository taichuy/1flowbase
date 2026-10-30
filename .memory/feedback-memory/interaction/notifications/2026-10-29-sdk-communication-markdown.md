---
memory_type: feedback
feedback_category: architecture_boundary
topic: 通讯消息格式属于共享 SDK 规范
created_at: "2026-10-29 15"
updated_at: "2026-10-29 15"
decision_policy: direct_reference
status: active
---

- 规则：用户纠正“把 /help 改成 Markdown 列表”的局部处理；通讯回复应作为统一 SDK 规范，由底座与插件贡献传入内容参数，在共享 owner 生成 Markdown。适用全部通讯回复，不逐命令或逐渠道维护相同格式逻辑。
- 原因：只修改 /help 会留下模型列表、提示与插件回复的同类换行问题；调用方应该表达内容语义，由共享契约处理渲染。
- 适用场景：通讯命令回复、底座提示、插件贡献输出及同类格式问题；已有 AI Markdown 明确标注并原样保留，不能猜测格式而二次转义。
- 证据入口：用户2026-10-29注释“不是/help 全部……列为SDK规范”；Single Issue #2704。交付和运行状态以当前代码、CI和进程证据复核。
