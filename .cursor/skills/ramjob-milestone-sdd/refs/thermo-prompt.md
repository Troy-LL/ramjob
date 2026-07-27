# Milestone thermo prompt

Dispatch **once** per milestone after all tickets are reviewer-approved and milestone verify passed.

Never per task.

## Task call

```
description: Thermo CQ review M<n>
subagent_type: thermo-nuclear-code-quality-review-subagent
prompt: |
  Full Repository Path: <REPO absolute path>
  Diff: branch changes
  Custom Instructions: |
    RamJob milestone M<n> end gate. Product name RamJob.
    Stack context. Rust + windows-rs for early milestones. Tauri only if this milestone already includes it.

    Scope. Review the milestone branch diff only.
    Focus. Maintainability, structure, 1k-line rule, spaghetti growth, code judo.
    Do not demand product-shape changes that contradict SPEC.md.
    Do not reopen M1 §9.3. That is a human gate.

    Milestone verify already ran. Prefer structural findings over re-litigating harness numbers.
```

Controller owns the follow-up.

1. Collect Critical and Important findings.
2. Fix via `poteto-agent` Tasks (serial if same tree).
3. Re-run milestone verify.
4. Record one thermo entry in `.superpowers/sdd/progress.md`.

**Done when.** Critical = 0. Important = 0 or human-waived. Verify still green. Exactly one thermo dispatch logged for this milestone.
