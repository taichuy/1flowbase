---
created_at: 2026-10-27 00
memory_type: project
decision_policy: verify_before_decision
scope: native distribution and issue 2683
---

# 原生便携发行试验

用户确认由 Root 创建并实施 Issue https://github.com/taichuy/1flowbase/issues/2683，动机是减少安装 Docker 的门槛。原生包保留唯一 Rust 后端并托管 Web 静态资源；随包 PostgreSQL 由后端管理，不需要 Nginx。镜像包含 Node.js 不等于产品运行必需，不将插件描述成额外后端。

固定要求：HTTP 默认 7800；PG 使用开发默认 35432，不使用常见 5432；包名仅产品、操作系统和 CPU 架构，不加 hash 或随机构建串。Windows EXE、Linux/macOS shell，程序与数据/密钥分离，保留 Docker 发行。截止日期未设定。

实现分支 codex/native-release-2683，提交 14063fa2c（启动/托管/打包）、4bc68eeaa（复用 Docker 固定版本的价格与应用模板资源）、fb23ee366（Windows 编译和直接子进程回收）。均已集成并推送 dev，合入 Windows 修复时主线为 5203c3ab1，后续以 git 核对。

本机 Linux x64 的 5 项原生 Rust 测试、2 项打包测试、6 项网络出口生命周期回归、11 项实际启停/认证/PG 导出恢复检查通过；新前端构建完成，浏览器实际进入自带示例页且无脚本/HTTP 错误。试验包 tmp/native-release/1flowbase-linux-x64.tar.gz 为本机 debug 构建，不冒充稳定发行。证据目录 tmp/test-governance/native-release-2683/。正常测试结束后回收临时 API/PG 与认证 session。

CI 的 Linux x64/ARM64 与 macOS ARM64 已取得通过证据。完整资源版构建 https://github.com/taichuy/1flowbase/actions/runs/40358060543；Windows 修复构建 https://github.com/taichuy/1flowbase/actions/runs/40359104074。动态状态回到当前 run 核对，不从旧快照推断 Intel Mac/Windows 成功。工作流现为手动选择平台，避免每次推送都执行五套 Release 冷构建。

Windows 网络出口插件当前只回收直接子进程，进程树退出收据保留未验证；Job Object、Windows 控制台关闭行为、签名/公证与全部插件支持仍须分别验收。Issue 保持开放，由用户验收。

## 2026-10-31 08 共享 Web 构建方向

用户在排查 Actions run 41146645075 后确认按平衡方案实施：Root 调整发布编排，一次构建 Web，并用同一次 run 的 artifact 供 Docker 双架构与全部五个平台便携包消费；修复 Windows 检出 CRLF 导致的工作流断言失败。动机是消除完整发行最多七次 Web 构建，而不只增加下载缓存。保留手动单平台验证入口、原有发行包与升级验证、发布失败门禁。

Rust 暂时保留 Debian 容器与原生 Runner 的编译环境、target 缓存隔离，增加缓存命中和编译耗时证据；Linux 后端二进制统一复用不在本次授权范围。当前主工作树分支为 beta；在线验证使用不带 release_tag 的 Windows 手动构建。截止日期未设定，成功口径为共享产物消费关系与换行符回归通过，Windows 实际编译和包验证另以当前 run 结果结算，不从结构测试推断跨平台运行成功。
