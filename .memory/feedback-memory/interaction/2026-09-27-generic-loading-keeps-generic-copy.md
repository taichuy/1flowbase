---
title: 复用通用加载态时保持通用文案
memory_type: feedback
feedback_category: interaction
created_at: 2026-09-27 16
updated_at: 2026-09-27 16
decision_policy: direct_reference
---
- 规则：用户要求使用统一通用加载态时，直接复用组件现有文案；未经明确要求，不为具体页面定制其加载文案，也不改动原有文案资源。
- 原因：页面专属长文案在通用加载组件的紧凑描述区域发生换行，破坏统一视觉。
- 适用场景：本项目将页面、侧栏或画布的局部加载提示替换为共享加载组件时。
