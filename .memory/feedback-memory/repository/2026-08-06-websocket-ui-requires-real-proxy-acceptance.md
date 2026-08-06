---
memory_type: feedback
feedback_category: repository
topic: WebSocket UI 必须经过真实浏览器与部署代理验收
summary: WebSocket 客户端、后端 route 和 mock 测试通过不能结算实时 UI；必须从真实页面验证 ticket、upgrade、首帧、client command、terminal、cancel 和代理转发。
keywords:
  - WebSocket
  - Vite proxy
  - real browser
  - streaming
  - cancel
  - runtime acceptance
match_when:
  - 实现或验收浏览器 WebSocket、SSE fallback、实时助手或双向运行控制
  - 单元测试通过但用户反馈页面一直 loading、无法停止或没有增量事件
created_at: 2026-08-06 11
updated_at: 2026-08-06 11
last_verified_at: 2026-08-06 11
decision_policy: direct_reference
scope:
  - web/app/vite.config.ts
  - web/packages/api-client/src/console-assistant.ts
  - web/app/src/features/agent-flow/hooks/useEmbeddedAssistantSession.ts
  - api/apps/api-server/src/routes/assistant/websocket.rs
---

# WebSocket UI 必须经过真实代理链路验收

## 规则

浏览器 WebSocket 交付不能只验证后端 upgrade fixture、API client mock、组件测试或 production build。必须用真实浏览器经过当前页面实际使用的代理 / ingress，至少观察：ticket 200、upgrade 101、服务端 ready 首帧、client command 发出、运行事件增量、terminal、handshake 超时回退、握手前停止，以及关闭窗口后的连接和运行释放。

## 原因

Issue #1601 的自动化测试和 AI Gateway gate 全部通过，但真实 Vite 页面把 WebSocket 指向前端同源代理；代理缺少 WebSocket 转发后，浏览器长期停在 CONNECTING。后端 route 直接连接正常，页面却既不发送 run.create，也不触发 error / SSE fallback，停止逻辑又因没有 run_id 提前返回。只有真实页面的 WebSocket frame timeline 暴露该集成缝隙。

## 适用场景

适用于实时助手、Native WebSocket、SSE fallback、浏览器通知、双向控制通道，以及任何依赖 dev proxy、reverse proxy、ingress 或同源升级的功能。
