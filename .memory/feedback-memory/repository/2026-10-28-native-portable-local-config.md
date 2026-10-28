---
memory_type: feedback
feedback_category: repository
topic: native-portable-local-config
summary: 原生便携包的配置应在解压项目目录内可直接找到，不默认让用户去 AppData 或用户目录找账号密码。
keywords: [native, portable, settings.json, data-directory, windows]
match_when:
  - 设计或调整 1flowbase 原生便携包的配置、数据位置及首次登录体验
created_at: 2026-10-28 22
updated_at: 2026-10-28 22
last_verified_at: 2026-10-28 22
decision_policy: direct_reference
scope: [native-distribution, portable-config]
---

## 规则

便携包默认把配置放在解压项目目录内，让用户在程序旁即可找到；不要默认把初始账号密码配置藏到 AppData、Library 或其他用户目录。路径以可执行文件位置确定，保持三平台一致。目录细分与迁移实现以当前代码和任务验收为准。

## 原因

用户成功启动 Windows 便携包后，明确指出让用户到用户目录寻找默认账号密码不合理，要求放在项目目录。便携包应符合解压、启动、就地查看配置的使用预期。

## 适用场景

原生包配置路径、首次登录说明、便携数据布局及升级迁移；调整默认位置时保留既有数据库、密码和加密密钥，不用新建空库掩盖旧数据。
