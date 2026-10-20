---
title: UI 管理统一控制页面设计入口与菜单配置
status: active
date: 2026-10-20 16
feedback_category: boundary_clarification
decision_policy: direct_reference
---

## 规则
用户明确要求：没有「UI 管理」权限时隐藏顶部「UI」及菜单配置入口，把 UI 菜单配置接口归入既有「UI 管理」权限组；不新增独立的「页面设计」角色配置入口。Root 保留超级管理员豁免。

## 原因
用户希望管理员从同一个权限分组控制 UI 设计与菜单配置，不接受仅解释现有设计权限与 UI 管理独立、再建议分别配置的方案。

## 适用场景
本轮 UI 管理授权收敛及后续相关入口、菜单配置、鉴权回归；接口仍应维持单操作授权，正常页面浏览不因缺少管理权限而整体关闭。
