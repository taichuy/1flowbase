# Design Rules

## Goal

把必要复杂度放到最理解业务语义、最靠近变化源的模块；避免用命名、分支、包装层或重复防御掩盖设计问题。

## Gate

方案新增公共抽象、接口、flag、通用 helper、重复校验或 pass-through 时检查本文件。命中停止信号后，先给更小 redesign，再进入 issue 或实现。

## Rules

1. **Use domain names.** 名称描述具体对象或动作；避免让 `data`、`result`、`handler`、`manager`、`process`、`utils`、`helper`、`*_impl` 承担领域语义。
2. **Validate at system edges.** 在 API、表单、外部协议、存储或插件边界校验；内部依赖明确 invariant，不散落重复防御。
3. **Keep interfaces narrow and implementations deep.** 公共入口保持简单，把状态判断和协议细节收敛在实现内部；不新增只转发参数的层。
4. **Model real variation explicitly.** 不用 bool / flag 为公共 API 增加特殊行为；真实差异使用独立概念、方法、状态或入口表达。UI 的 `open`、`disabled`、`loading` 等自然状态不受此限制。
5. **Abstract after evidence.** 只有重复语义、稳定 invariant 或外部依赖边界已经出现时才抽象；不为“以后可能复用”提前创建 wrapper、adapter 或 manager。
6. **Comment decisions, not syntax.** 注释记录原因、约束、外部要求或舍弃方案，不复述代码。
7. **Prefer mature mechanisms.** 成熟算法、数据结构、状态机或约束能更直接表达问题时优先复用；不能降低复杂度或被验证时不采用。

## Stop Signals

- 新增抽象却说不清 owner、invariant 或隐藏了什么复杂度。
- 单个特殊 case 迫使多个调用方增加分支或兼容约定。
- 同一校验、fallback 或状态判断开始在内部路径重复。
- 新层只改名或转发参数，没有承担领域责任。
- 自定义规则正在替代已有成熟算法、数据结构或约束表达。

## Blocked Output

三句以内说明触发的规则、复杂度如何扩散，以及更小 redesign。证据不足时回到需求对齐，不用抽象原则压过代码事实。
