---
title: 登录表单受限网络加载优化已批准
created_at: 2026-09-08 10
updated_at: 2026-09-08 10
decision_policy: verify_before_decision
status: active
---

用户批准当前 Root 执行平衡方向：登录场景加载调度、有限预取、编译 Singleflight/LRU 与必要依赖隔离；Single Issue https://github.com/taichuy/1flowbase/issues/2012 。用户补充公网 https://1flowbase.taichuy.cn/ 与本地 3100 同实例，目标是在资源受限条件下观察和优化，未指定截止日期。

实现保持认证/表单源码与后端 contract；按真实依赖图分离 browser 与 compiler-worker 入口，未进入发布时编译。成对代理限速样本公网首次中位值 43.869→37.818 秒，重复进入 4658→158.5ms；本地首次 41.339→40.854 秒，基本无改善。不能把本阶段说成冷启动或宕机问题全部解决。当前用户验收阶段，继续先核对 Issue 和 tmp/test-governance/auth-loading/report.md；测试预算与配置必须复用记录，不混用早期 CDP/重载/503 探索样本。
