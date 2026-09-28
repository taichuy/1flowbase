---
memory_type: feedback
feedback_category: interaction
topic: API 慢请求归因需要多轮与间隔对照证据
summary: 用户要求在排除后端前核对真实请求、多轮结果、间隔后的行为和缓存；不能以第二次快或源码直读数据库推断后端无关。
keywords:
  - API latency
  - repeated requests
  - cold start
  - cache
match_when:
  - 用户反馈偶发 API 慢、首次请求慢或再次打开变快
  - 准备排除后端、数据库、缓存或网络链路
created_at: 2026-09-28 09
updated_at: 2026-09-28 09
last_verified_at: 2026-09-28 09
decision_policy: direct_reference
---

# Rule

先测用户指定接口，区分本地后端与公网路径，保留多轮、新建 / 复用连接、间隔后请求及缓存对照的证据。能够采集时，用同一公网请求在后端入口的接收 / 响应时间支持归因；核对测试工具的代理路径。

结论说明已覆盖的条件和未验证边界：接口间隔不等于进程、数据库或连接池真正冷启动；源码没有结果缓存也不能排除数据库和操作系统缓存。没有对应证据时不说“和后端无关”。

# Reason

用户纠正了以首次 / 再次速度差异直接排除后端的判断，要求判断能够由运行态证据核验。

# Applies When

偶发慢请求、页面首载延迟和缓存 / 冷启动诊断；用户明确要求保留的现有进程不能为取证而重启或清缓存。
