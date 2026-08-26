---
memory_type: feedback
feedback_category: repository
topic: Native React 模块类型直接来自构建实际解析的组件依赖
summary: Native React 编辑器类型注册不得手工绑定组件版本或按版本维护声明；应从当前构建实际解析的依赖声明中自动注册。
keywords:
  - native-react
  - Monaco
  - type declarations
  - antd
  - module registry
match_when:
  - 调整 Native React 编辑器模块类型声明
  - 注册前端组件依赖的运行时导出与 TypeScript 类型
  - 处理第三方组件升级后的 Monaco 类型漂移
created_at: 2026-08-26 17
updated_at: 2026-08-26 17
last_verified_at: 2026-08-26 17
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage/lib/native-modules
  - web/app/src/shared/code-block
---

# Native React 模块类型直接来自构建依赖

## 时间

`2026-08-26 17`

## 规则

Native React / Monaco 的组件类型声明不在产品代码中写组件版本，也不为不同版本手工维护声明分支。模块注册以当前构建实际解析到的组件依赖及其 `.d.ts` 为唯一类型真值；依赖升级后由构建重新生成或装载声明。

## 原因

用户明确要求不要绑定组件版本，并指出组件既然已经是项目依赖，类型能力就应直接从该依赖注册。运行时 `Object.keys()` 只能发现值导出，类型导出必须在构建期从依赖声明图读取，但不需要额外版本字段或版本分支。

## 适用场景

- Ant Design、React、Ant Design X 等已注册 Native React 模块的 Monaco 类型供应。
- 修复 `FlexProps`、`DividerProps`、`GetProp` 等 type-only export 缺失。
- 升级组件依赖后保持编辑器类型与实际构建依赖一致。
