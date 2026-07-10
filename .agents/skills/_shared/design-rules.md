# Design Rules

## Goal

在需求进入 issue 或实现前，先拦住 AI 常见的过度设计：模糊命名、重复防御、无意义包装、bool 分支、pass-through 层和提前抽象。

## Gate

改产品代码前先检查本文件。若请求或方案会违反任一规则，停止并先说明更小的 redesign；用户确认后再创建 / 更新 issue 或进入实现。

## Rules

1. **Names must disambiguate.**
   禁止默认使用 `data`、`info`、`result`、`handler`、`manager`、`process`、`utils`、`helper`、`do_*`、`*_impl` 作为类型、函数、文件、模块、接口字段或跨 10 行以上变量名。需要使用时，改成描述具体对象或动作的名字。

2. **Validate once at system edges; trust explicit invariants inside.**
   在 API、表单、外部协议、存储读取、插件边界等系统边缘校验一次；内部边界依赖明确 invariant。不要在可信内部路径散落重复 defensive check。相同 `None/null/empty/status` 校验出现 3 次以上时，先重设边界，不要默认新增 wrapper type。

3. **Comments say WHY, not WHAT.**
   注释只说明意图、约束、舍弃方案、外部要求或非显然原因。不要复述代码正在做什么。

4. **No flag/bool parameter for special-case behavior.**
   新增 public/shared function、service、hook、adapter 或组件 API 时，不用 bool/flag 参数承载特殊分支。若 variation 是真实概念，优先拆成独立概念、方法或入口。UI 状态字段如 `disabled`、`open`、`loading` 不属于本规则目标。

5. **Narrow interfaces, thick implementations.**
   新增 public function/method/parameter 前，先说明为什么现有抽象不能吸收复杂度。不要创建只转发参数的 pass-through 层；只有在隐藏复杂度、执行 invariant 或适配外部依赖时才新增层。

## Stop Signals

- 方案需要新增 `manager/helper/utils` 但说不清具体职责。
- 为单个特殊 case 增加 bool 参数。
- 为了“以后可能复用”新增抽象、wrapper、adapter 或 pass-through。
- 同一校验在多个内部函数重复出现。
- 注释能被代码本身直接读出来。

## Output When Blocked

用三句以内说明：

- 哪条 rule 被触发。
- 当前方案为什么会扩大复杂度。
- 更小 redesign 是什么。
