---
memory_type: feedback
feedback_category: repository
topic: Compose 部署配置初始化的评估边界
summary: 评估单文件 Compose 时先考虑已有宿主挂载目录内初始化配置，区分 Compose 插值与容器配置加载，以及 Cookie 和网络访问限制。
created_at: 2026-10-07 11
updated_at: 2026-10-07 11
decision_policy: direct_reference
scope:
  - docker-deployment
---

# 规则

用户要求评估沿用挂载目录、由容器初始化配置，并在存在 .env 时使用其配置、缺失时采用默认值的部署路径。不能把命名卷描述为唯一持久化方式，也不能把 Compose 插值的时序限制解释为容器无法读取配置文件。

# 原因

宿主目录 bind mount 同样支持持久化；初始化和配置读取可以由启动入口承担。Secure Cookie 的浏览器行为与 API 端口暴露、CORS、鉴权分别属于不同机制，解释时应明确区分。

# 适用场景

1flowbase 单文件 Compose 部署方案讨论。此反馈不代表用户已批准实现或使用固定的生产默认密码与加密密钥。

2026-10-07 11 用户已确认 Docker 部署默认使用 HTTP，保留 HTTPS Cookie 配置覆盖，并由用户在 Wiki 说明环境配置文件。本次授权仅落实 HTTP 默认值，尚未批准完整的容器配置初始化改造。
