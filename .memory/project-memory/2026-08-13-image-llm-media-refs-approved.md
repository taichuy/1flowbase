---
memory_type: project
topic: 挂载视觉模型通过 media_refs 显式消费会话图片
summary: 用户确认 image_llm 使用必填 media_refs 显式选择会话图片，不隐式继承最近图片、不落默认文件管理；已发布目标工具配置，Single Issue #1680 是实现与验收真值。
keywords:
  - image_llm
  - media_refs
  - base64
  - visible internal llm tool
  - multimodal
created_at: 2026-08-13 21
updated_at: 2026-08-13 22
last_verified_at: 2026-08-13 22
decision_policy: verify_before_decision
scope:
  - api/crates/orchestration-runtime
  - api/crates/control-plane/src/orchestration_runtime/provider_invoker
  - application 019f5443-5b8e-74b2-90e3-c867dbddd37b
---

# Image LLM Media Refs

## 当前决定

用户已确认 `image_llm` 工具使用必填 `media_refs: string[]` 显式选择当前会话可见图片。后端生成 opaque 引用并解析回已有原始 content block，再注入挂载的原生多模态 Provider；不复制 base64，不依赖挂载模型继承父模型上下文，不把图片写入默认文件管理，也不回退为 `workspace_path` 或隐式最近图片。

用户已在 `/applications/019f5443-5b8e-74b2-90e3-c867dbddd37b/orchestration` 发布目标配置：`media_content_available(image)` 的 `argument_path` 为 `media_refs`，工具输入必填 `task` 与 `media_refs`。

## 动机与证据

基线 run `019ffa29-60c2-7852-aabf-837ebcd4fdd2` 已收到并持久化 base64 图片，但旧工具 schema 只接受 `workspace_path`，文本模型因而调用客户端 `Shell` 搜索文件，内部 `image_llm` 路由没有执行。

## 在线真值与截止口径

Single Issue [#1680](https://github.com/taichuy/1flowbase/issues/1680) 是实现、QA 与用户验收的唯一计划真值。实现已通过 commit `5158d58f9` 合入本地 `dev`，自动化 AC-001～AC-005 已通过，等待用户在关联应用结算 AC-006。完成口径是仅有 base64 图片的真实请求能够通过显式 `media_refs` 触发内部视觉工具并得到视觉结果，且不产生用于寻找图片的 `Shell` / `Read` callback。

GitHub CLI 凭据在实现结束时失效，因此线上 Issue 尚未从 `phase:ready` 更新为 `phase:user-acceptance`，证据评论也尚未回写；重新认证后应补齐，但不影响本地代码与测试结果。
