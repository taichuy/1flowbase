# Paipan chat workflows

[English](README.en.md)

八字与紫微使用两条独立指令、两个聊天事件、两次工作流，各返回一份完整报告。

```text
/paipan --brith=2026-11-11 --time=13:09:00 --gender=男 --country=中国 --province=广东 --city=湛江市
/ziwei --brith=2026-11-11 --time=13:09:00 --gender=男 --country=中国 --province=广东 --city=湛江市
```

六项全部必填，保留 `brith` 拼写；性别填写男/女，国家填写中国，省、市分别使用汉字参数。默认真太阳时、UTC+08:00；地址解析取得城市参考坐标。运限日期时间使用执行时刻。

## Generate drafts

在仓库根目录生成两个原生应用导入包：

```bash
node docs/architecture/event-center/examples/paipan-chat/build-workflow.mjs tmp/paipan-bazi.json YOUR_WECHAT_INSTANCE_UUID bazi
node docs/architecture/event-center/examples/paipan-chat/build-workflow.mjs tmp/paipan-ziwei.json YOUR_WECHAT_INSTANCE_UUID ziwei
```

`bazi` 为省略最后参数时的默认值。生成器没有密钥，不自动发布、注册命令或发送微信。

```text
/paipan → 八字事件 → 地址/出生/农历/真太阳时 → bazi/calculate → 八字报告 → Notification
/ziwei  → 紫微事件 → 地址/出生/农历/真太阳时 → ziwei/calculate → 紫微报告 → Notification
```

两个工作流各自校验参数、计算、处理错误和发送对应报告；不会串联运行另一个排盘。八字正文包含四柱、藏干、十神、五行、大运及流年；紫微正文包含基础信息、十二宫、主辅星、四化、大小限及运势。紫微年龄展示从自己的出生/参考日期投影，不请求八字命盘。

## Configure

1. 导入两个包为事件工作流，分别创建“八字排盘请求”和“紫微排盘请求”聊天指令事件。
2. 渠道“聊天指令”表分别配置 `user/paipan`（名称 `paipan`）和 `user/ziwei`（名称 `ziwei`），各选对应事件。四个来源字段使用后端固定映射。
3. 两个工作流各选自己的 `published` 事件，映射 `arguments/input_id/conversation_id/instance_id` 到 `/payload/` 下同名字段。
4. 分别配置 `CalendarApiOrigin`、`CalendarApiAuthorization`。使用专用受限日历账号的 API key；包不发放密钥、不保存 root cookie。凭据和 HTTP 调试数据可见范围沿用应用权限。
5. 检查计算结果与报告，发布后启用对应指令。真实 Notification 会发送消息；草稿验证可替换为返回文本的 Code 节点，必须标记未发送。

## Channel history and receipts

发送经过对应通道插件。支持标准发送回执的通道声明 `receive.supports_send_receipts`；Host 给出请求身份，SDK校验关联，插件返回真实外部收件会话及可用的供应商/client标识。Host统一归档会话历史，普通AI和指令回复复用已有预留记录，避免重复；主动发送没有伪造的入站输入。

事件工作流的回复目标从原始持久指令恢复，不能从可编辑的工作流参数获得授权。来源渠道不匹配时停止，不能把私聊示例直接推广为任意群聊/广播。

事件受理、工作流完成、供应商受理分别观察。`accepted`不代表已读；未知结果不自动重发。成功受理不发送技术回执；支持临时活动的插件在执行期间显示原生输入状态，状态不是聊天正文。

AI通过通道 `history_messages` 按会话和发送者作用域分页查询历史，不自动将整段历史回放到每次请求。旧发送日志没有可靠会话关联时不猜测回填。

发送日志终态、标准回执和历史消息使用同一事务提交。已校验回执由独立收尾任务保留；临时存储故障只重试本地归档，不重新调用供应商。原输入/回复轮次已恢复为未知，或所属工作流已经结束而没有持久回执时，历史将未完成发送投影为未知，不能继续显示发送中。进程在事务提交前崩溃仍可能无法确认供应商结果，不据此重发。

## Mock draft

`build-mock.mjs` 是早期六参数固定样例验证，输出 `mock:true,sent:false`；不证明真实事件、计算或微信收件。当前独立链路验证应使用两个独立工作流及明确隔离的模拟通道，不能将同一命令双报告旧实现作为预期。
