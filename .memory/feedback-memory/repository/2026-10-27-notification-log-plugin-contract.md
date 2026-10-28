---
memory_type: feedback
feedback_category: repository
topic: notification-log-plugin-contract
summary: 发送日志由插件按协议声明标题、正文和JSON附属信息，宿主不手工适配供应商字段。
keywords: [notifications, communications, plugin, logs, JSON]
match_when:
  - 设计或修复通讯发送日志及插件日志接入
created_at: 2026-10-27 21
updated_at: 2026-10-27 23
last_verified_at: 2026-10-27 23
decision_policy: direct_reference
scope: [notification-contract, notifications, wechat-chat-channels]
---

## 规则

插件通过统一协议声明发送消息的日志语义；发送日志采用标题、完整正文、JSON附属信息三个部分，不把消息正文仅塞进泛化输入/输出JSON，也不由宿主按微信等供应商手工适配字段。缺失或空白标题时从正文取前255个Unicode scalar字符作为标题，正文完整保存；不要按字节截断。

## 原因

用户纠正了只增加通用JSON快照的方案：希望保留明确可读的消息结构，由插件承担字段语义，底座统一落库与展示。

## 适用场景

发送日志模型、通知类型声明、插件日志投影和详情展示。此条记录用户确认的设计偏好；具体实现与部署状态以当前源码和运行态为准。
