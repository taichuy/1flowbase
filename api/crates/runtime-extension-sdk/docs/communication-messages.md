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
