# `.memory` 规则

`.memory` 是开发者私有记忆目录，用来记录 AI 与开发者之间可跨轮复用的偏好、纠正、阶段决策、外部入口和工具失败解法。

所有日期统一使用绝对时间格式：`yyyy-mm-dd hh`。

## 启动读取

1. 进入用户交互后，固定先读 `.memory/AGENTS.md`。
2. 如存在 `.memory/user-memory.md`，固定再读该文件。
3. 如 `.memory/user-memory.md` 缺失或为空，参考 `.memory/user-memory.template.md` 与用户沟通初始化；模板本身不作为有效用户记忆。

## 检索预算

1. 对 `feedback-memory`、`project-memory`、`reference-memory`、`tool-memory`，第一轮只读取每个文件前 30 行 YAML front matter，并跳过 `TEMPLATE.md`。
2. 单轮最多扫描 200 个记忆文件。
3. 单轮最多展开 5 条最相关有效记忆全文。
4. 有效记忆宁缺毋滥；达到当前任务所需证据后立即停止检索。
5. 不为措辞、全面感、凑满配额或重复佐证继续展开记忆。
6. `.memory/project-memory/archive/` 只在当前有效记忆缺失、正文不足或需要追溯同主题旧阶段时读取。

## 取舍规则

1. 是否采用记忆须同时满足范围、授权与新鲜度：适用于当前仓库 / 环境 / 任务阶段及对象；未超出当前明确授权；影响决策的事实仍由当前代码、文档或运行结果支持。记忆类型、`decision_policy` 和时间只辅助判断，不把旧阶段决定或偏好当作当前授权。
2. `project-memory` 超过两天且影响当前决策时，回到当前来源验证；两天内也不自动有效，基线、环境、权限或阶段已变时同样核对。无法验证的动态事实只作线索，不作为当前事实或验收证据。
3. `reference-memory` 只作为入口索引，不直接作为结论来源。
4. `tool-memory` 只在当前任务会用到对应工具，或该工具刚刚失败时参与检索。
5. 记忆与最新 AGENTS、README、代码、运行结果冲突时，以最新可验证来源为准，并在必要时更新记忆。

## 写入触发

每轮最终回复前静默判断是否需要维护记忆。命中触发器时，先写入或更新记忆，再最终回复。

### 用户记忆

写入本地私有 `.memory/user-memory.md`（不提交；格式参考 `.memory/user-memory.template.md`）：

- 长期稳定偏好。
- 协作方式。
- 技术背景。
- 决策习惯。
- 沟通风格。

典型话术：`以后都`、`我更喜欢`、`不要再`、`默认按`、`我习惯`。

### 反馈记忆

写入 `.memory/feedback-memory/interaction/` 或 `.memory/feedback-memory/repository/`：

- 用户对 AI 做法给出纠正、否定、认可或边界澄清。
- 每条必须包含：规则、原因、适用场景。
- 既记录纠正，也记录肯定，避免 agent 越来越保守。
- YAML front matter 必须包含 `feedback_category`。
- `decision_policy` 默认使用 `direct_reference`。

典型话术：`你这里不应该`、`这样不对`、`这个方向是对的`、`以后遇到这种情况`、`不是 X，而是 Y`。

### 项目记忆

写入 `.memory/project-memory/`：

- 用户确认阶段性方案。
- 架构方向。
- 当前任务边界。
- 短期状态。
- 已批准计划。

正文至少回答：谁在做什么、为什么这样做、为什么要做、截止日期、决策背后动机。

`decision_policy` 默认使用 `verify_before_decision`。

### 引用记忆

写入本地私有 `.memory/reference-memory/`（不提交；格式参考 `.memory/reference-memory/TEMPLATE.md`）：

- 用户提供仓库外资料、外部路径、参考项目、网页或文档入口，且后续可能复用。
- 只记录“去哪里看什么”的入口索引，不记录正文结论。
- `decision_policy` 默认使用 `index_only`。

### 工具记忆

写入本地私有 `.memory/tool-memory/`（不提交；格式参考 `.memory/tool-memory/TEMPLATE.md`）：

- 项目环境中发生真实工具失败。
- 本轮验证出可复用解决办法。
- 只记录真实失败、已验证解法和复现处理入口。
- 不写通用工具使用文档或未来风险提示。
- 同一工具、同一问题、同一处理办法复现时，追加到原文件。
- `decision_policy` 默认使用 `reference_on_failure`。

## 写入预算

1. 单轮最多新增或更新 3 条记忆。
2. 已有同主题记忆可承载时，优先更新旧文件，不重复新建。
3. 长期用户偏好优先级高于单次项目事实。
4. 反馈记忆优先级高于项目记忆。
5. 30 天后仍有效的信息，优先沉淀为用户记忆、反馈记忆或稳定规则；否则保持项目记忆。
6. 完成必要写入后停止，不为措辞、全面感或归档洁癖继续扩写。

## 不写入

1. 可直接从当前代码中读取的代码模式、架构和路径。
2. 版本管理历史；优先回看 `git`。
3. 一次性调试过程或一次性修复流水账。
4. 配置文件中已经明确声明的内容。
5. 临时任务细节。
6. 与记忆目录无关的运行摘要、草稿或历史杂项文件。
7. 已经被 AGENTS、README、skill 或代码稳定承载的规则复述。

## 目录索引

- `.memory/user-memory.md`：用户记忆主文件，本地私有，不提交；模板见 `.memory/user-memory.template.md`。
- `.memory/feedback-memory/`：反馈记忆根目录。
- `.memory/feedback-memory/interaction/`：沟通、执行流程、记忆检索等交互纠正。
- `.memory/feedback-memory/repository/`：仓库结构、目录管理、脚本放置、版本控制等工程纠正。
- `.memory/project-memory/`：当前有效项目记忆。
- `.memory/project-memory/archive/`：旧阶段项目记忆归档。
- `.memory/reference-memory/`：引用入口索引，本地私有，不提交；模板见 `.memory/reference-memory/TEMPLATE.md`。
- `.memory/tool-memory/`：真实工具失败与已验证解法，本地私有，不提交；模板见 `.memory/tool-memory/TEMPLATE.md`。
- `.memory/todolist/`：AI 自主开发且用户离线时的待确认事项，本地私有，不提交；模板见 `.memory/todolist/TEMPLATE.md`，处理后删除。
- 各目录中的 `TEMPLATE.md` 只作格式示例，不作为有效记忆检索输入。

## 目录外排除

`.memory/` 根目录下不属于 `AGENTS.md`、`user-memory.md` 和指定记忆目录的其他文件，不作为记忆检索输入。
