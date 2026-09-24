---
date: 2026-09-24 17
feedback_category: interaction
decision_policy: direct_reference
---

# Agent Logs Capture: Favor Small Universal Integration

- 规则：评估 agent-logs 双向流量采集时，优先比较普适性和对现有 `NGINX → 7800` 路径的最小改动；对 Langfuse、LiteLLM 等完整网关或观测系统，先说明新增链路和运行成本，再判断是否合适。用户提出 mitmproxy 时，具体评估它的反向代理和流式采集边界。
- 原因：用户认为引入 Langfuse 或 LiteLLM 太重，希望用较小的通用采集层满足请求、响应和入库需求。
- 适用场景：agent-logs 的架构决策、NGINX 流量镜像与请求/响应采集方案比较。
