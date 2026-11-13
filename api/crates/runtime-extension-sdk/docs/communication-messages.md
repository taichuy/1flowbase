# 通讯消息输出规范

[English](communication-messages.en.md)

底座与插件贡献使用同一份 `extension-contracts` 内容类型和 Markdown renderer；SDK 直接重导出这些 API。规范覆盖帮助、模型列表、成功/错误提示与插件回复，不为每个命令或渠道各写格式化规则。

## 内容参数

| 类型 | 用途 | 渲染规则 |
| --- | --- | --- |
| `CommunicationMessageContent::Text` | 普通文本、动态字段、错误说明 | 转义 Markdown 特殊字符；LF/CRLF 转成明确的 Markdown 硬换行 |
| `CommunicationMessageContent::Blocks` | 按参数构建段落和列表 | 段落间空行分隔；列表项生成 `- `，多行内容仍属于原条目 |
| `CommunicationMessageContent::Markdown` | 已有 Markdown，包括 AI 最终答案 | 原字节保留，不猜测格式，不二次转义或替换换行 |

`CommunicationMessageBlock` 提供 `Paragraph`、`BulletList`；`CommunicationMessageInline` 提供 `Text`、`Strong`、`Code`。命令用法、模型 ID 等可复制值用 `Code`；名称和说明用 `Text`。SDK 根据值中的反引号选择正确代码分隔符，不要求调用方手工转义。

普通段落的单换行可能被 Markdown 客户端合并。独立条目应传入 `BulletList`，不能依赖把几行普通字符串拼在一起。

## 插件示例

```rust,no_run
use runtime_extension_sdk::{
    serve_communication_command, CommunicationMessageBlock as Block,
    CommunicationMessageContent as Content, CommunicationMessageInline as Inline,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    serve_communication_command(std::io::stdin().lock(), std::io::stdout().lock(), |request| {
        Content::Blocks(vec![
            Block::Paragraph(vec![Inline::Strong("Result".into())]),
            Block::BulletList(vec![vec![
                Inline::Code(request.command_id.clone()),
                Inline::Text(format!(" — {}", request.arguments)),
            ]]),
        ])
    })?;
    Ok(())
}
```

简单回复可直接返回 `String` 或 `&str`，SDK 按 `Text` 处理。已有 Markdown 显式返回 `Content::markdown(markdown)`。使用通用事件 worker 时，通过 `CommunicationCommandReply::from_content(content)?` 构造 `ManagedEventOutcome::CommandReply`，并按 handler 的既有错误处理返回失败。

## Wire、长度与所有权

内容树是作者侧 API；线上回包仍为 `CommunicationCommandReply { text }`，`text` 是已经渲染的 Markdown。命令 contract `1`、managed-event v2、持久化 `reply_json` 均保持原格式。现有回调返回 `CommunicationCommandReply` 仍受支持，按已有 Markdown 保留；SDK 无法从旧 `{text}` 自动区分纯文本和 Markdown，旧插件应改用上述内容参数才能获得明确的文本换行语义。

命令回复在渲染后按 UTF-8 字节校验，非空且不超过 32,768 字节。超限返回 `CommunicationCommandReplyContentError::TooLong`，SDK 不发送部分帧或截断代码分隔符。底座长命令/模型目录按完整条目缩减，并追加目录提示。`render_markdown` / `into_markdown` 本身没有命令长度限制；普通 AI 最终答案不套用该限制。

生成端只渲染一次。宿主持久化与发送使用同一成品正文；渠道插件按原文交给供应商。接收者、会话、权限、事件目标和发送收据由宿主负责，格式参数不授予新权限。附件仍使用原来的文件协议。


## 发送关联与供应商收据

接收型频道通过 `receive.supports_send_receipts: true` 声明发送收据能力；缺省为 `false`，序列化时省略 `false`。Host 仅向声明能力的频道发送 `NotificationOperation::Send.request_id`，值为 Host 发送日志 UUID。该字段缺省、省略时仍使用原发送协议。插件不得自行声明 Host 会话或发送者 UUID。

`NotificationRuntimeResult::Send.receipt` 是独立于业务 `outcome.result` 的 `NotificationSendReceipt`：

| 字段 | 来源与含义 |
| --- | --- |
| `request_id` | 原样回传本次 Host 请求 UUID |
| `recipient_id`、`conversation_id` | 本次实际发送的供应商收件人和会话身份；有 `target` 时分别匹配其 `sender_id`、`conversation_id` |
| `provider_message_id` | 可选，供应商实际签发的消息 ID |
| `client_message_id` | 可选，实际发包携带的客户端消息 ID；不得冒充 `provider_message_id` |

所有身份字段非空、无控制字符且不超过 1024 UTF-8 字节；`request_id` 还必须是 UUID。使用 `notification_send_receipt(request_id.as_deref(), recipient, conversation, provider_id, client_id)?` 共用构造与字段校验，`NotificationSendReceipt::validate_for` 校验请求和目标关联。`execute_communication` 会完成 descriptor、input 和 result 的整体校验。传入 `request_id: None` 时 helper 返回 `None`，旧 Host 不收到新 receipt 字段。

私密授权状态中的账号身份通过可选 `authorization.managed_identity_keys` 声明，必须是 `managed_secret_keys` 的无重复子集，值为有界身份字符串（null 仅用于删除）。它们继续加密存储；Host 只在请求及目标关联已验证的标准回执 `recipient_id` / `conversation_id` 位置允许原样投影。消息正文、其他业务结果、供应商/client 消息标识和其他公开输出仍受保护；密码、令牌和未声明身份的私密状态不会获得豁免，即使其值恰好等于账号身份。旧声明默认空列表，保持原检查。

带 `request_id` 的成功结果必须有匹配 receipt。`Failed` 和 `Unknown` 可以没有 receipt；若提供，仍必须匹配同一次请求与目标。未声明能力不能接收 `request_id`，没有 `request_id` 的结果不能返回 receipt。无效回包或供应商错误沿用 `Unknown`，不自动重发。供应商 accepted 仅表示接受此次提交，不表示 delivered 或 read；Host 统一关联并归档各入口产生的发送历史。


## 接收引用与历史检索

`NotificationReceivedMessage.reply_to` 是可选 `NotificationMessageReference`，含可选的供应商 `provider_message_id`、`title` 与引用 `text`。至少一项非空，身份使用原有1024字节限制，标题最多1024字节，正文最多65536字节，仍受整批接收512KiB约束。省略时保持原接收格式；不能携带Host的message_id、角色、凭据或私有下载描述符。插件负责提取供应商引用，不把引用正文拼进用户原指令。

Host在接收事务保存引用，只在同workspace/channel/sender/conversation内按原入站供应商ID或已保存发送收据的provider/client ID解析唯一原消息；可验证时补Host message_id及原正文，否则保留供应商引用快照，不根据标题或正文猜测关联。后续用户消息把引用作为用户数据提供给模型，不提升为system指令，不伪造文件附件。统一历史返回同名reply_to，控制台可展示引用。

历史工具每页最多5条；查找较早报告时，应沿next_cursor继续，直到找到目标或历史结束，不能仅因第一页没有就断言不存在。接收记录的send_status投影关联同一input对应的实际通道发送；指令处理状态仍表示接收/指令处理结果，和报告发送状态独立。没有真实回执的旧unknown保持不变。
