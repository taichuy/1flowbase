---
feedback_category: interaction
decision_policy: direct_reference
updated_at: 2026-09-22 07
---
# 日志等待超时只结算投影

规则：waiting_callback 的会话占位显示原状态文字。等待五分钟后只结算本地会话投影的费用与 token，展示最后模型正文，没有正文显示 Timeout。迟到回调仍可更新同一投影，不能重复累加或把超时视作取消。

原因：客户端停止不代表工作流取消；日志是可更新的只读业务投影，不能以展示收尾篡改工作流、节点、回调或实际扣费状态。

适用：应用任务日志与聊天历史的等待展示、超时投影、迟到回调。
