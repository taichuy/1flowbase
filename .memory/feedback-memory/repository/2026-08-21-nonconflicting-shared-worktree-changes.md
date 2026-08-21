---
memory_type: feedback
feedback_category: repository
topic: shared worktree unrelated edits
summary: Nonconflicting edits from another developer must not block the current packet; only assess overlap and commit them together when the user expressly authorizes it.
keywords:
  - shared worktree
  - unrelated edits
  - packet assembly
  - commit
match_when:
  - A shared worktree contains uncommitted changes outside the current packet.
created_at: 2026-08-21 17
updated_at: 2026-08-21 17
last_verified_at: 2026-08-21 17
decision_policy: direct_reference
scope:
  - git worktree
  - packet assembly
---

# Nonconflicting shared-worktree edits

## Rule

Do not pause or ask the other developer to restore unrelated edits merely because they exist in a shared worktree. Check only whether they overlap or conflict with the current packet. Include them in a joint commit only after the user expressly authorizes it.

## Reason

The user clarified that nonconflicting work by someone else does not affect the current development flow, then authorized the formatting-only support change to be committed with the candidate.

## Applicable scenario

Concurrent packet assembly or QA in a shared workspace.
