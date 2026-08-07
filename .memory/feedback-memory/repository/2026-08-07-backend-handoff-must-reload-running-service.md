---
memory_type: feedback
feedback_category: repository
topic: 后端人工测试交付必须确认常驻服务已加载当前代码
summary: 后端行为变更合并并通过 Cargo 测试后，若交付用户进行本地人工测试，必须确认常驻 api-server 的启动时间和可执行文件对应当前提交；测试二进制通过不代表正在运行的服务已热更新。
keywords:
  - api-server
  - backend handoff
  - stale process
  - deleted executable
  - manual testing
match_when:
  - 后端代码合并后交付用户在本地页面人工测试
  - 运行结果仍呈现修复前行为
  - Cargo 测试通过但常驻 api-server 未重启
created_at: 2026-08-07 07
updated_at: 2026-08-07 07
last_verified_at: 2026-08-07 07
decision_policy: direct_reference
scope:
  - api
  - scripts/node/dev-up.js
  - repository-workflow
---

# 后端人工测试交付必须加载当前服务版本

## 规则

后端行为变更合并并通过测试后，若下一步是用户在当前本地环境人工测试，交付前必须核对常驻 `api-server` 是否在目标提交之后启动，且 `/proc/<pid>/exe` 不是已被替换的旧 `(deleted)` inode。必要时使用项目的 backend restart 入口重新加载代码，再以新会话或运行制品取证。

## 原因

`cargo test` 编译和运行的是测试二进制，不会热替换已经启动的 `api-server` 进程。磁盘二进制即使被后续构建覆盖，旧进程仍会继续执行旧 inode，导致代码与测试均已修复但页面人工测试仍呈现旧行为。

## 适用场景

Assistant、MCP、API contract、权限、运行时适配器等后端变更合并后的本地人工验收，以及任何“最新代码仍表现为旧行为”的诊断。
