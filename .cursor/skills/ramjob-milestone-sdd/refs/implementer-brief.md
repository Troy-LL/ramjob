# Implementer and task-reviewer briefs

Controller fills placeholders. Every `Task` uses `subagent_type: "poteto-agent"`.

Default. Do **not** commit. Stage only if useful. Commit only when the user already approved commits for this session.

Work from the RamLimiter repo root. Product is RamJob. M0 stack is Rust + windows-rs.

---

## Implementer Task

```
description: Implement <NN> <ticket title>
subagent_type: poteto-agent
prompt: |
  You are implementing ticket <NN> <title>.

  ## Brief

  Read first: <BRIEF_FILE path under .scratch or .superpowers/sdd/>
  Ticket file: .scratch/ramjob/issues/<NN>-<slug>.md
  SPEC truth: SPEC.md (current milestone only)

  ## Context

  <Where this fits. Dependencies. Crate/tree you own. What must stay untouched.>

  ## Before you begin

  If requirements, approach, or dependencies are unclear, ask now. Do not guess product shape.

  ## Job

  1. Implement exactly what the ticket acceptance criteria require.
  2. Prefer TDD when the ticket implies testable behaviour.
  3. Verify against the ticket's Verify section on a real artifact.
  4. Do not commit unless the brief says the user approved commits this session. Otherwise stage and stop.
  5. Self-review for completeness, YAGNI, and pattern fit.
  6. Report status. DONE | DONE_WITH_CONCERNS | NEEDS_CONTEXT | BLOCKED.

  ## Boundaries

  - Own only the named crate/tree. No drive-by edits elsewhere.
  - No thermo-nuclear review. That is a milestone-end controller step.
  - No M3 to M6 work. No silent SPEC pivots.
  - Update nothing in other agents' trees.

  ## Report shape

  - Status
  - Files touched
  - Verify evidence (command + result summary)
  - Concerns or blockers
  - Commit SHAs only if commits were approved and made
```

Write the brief file the prompt points at before dispatch. Keep the controller free of full diffs.

---

## Task reviewer Task

Dispatch after implementer DONE or DONE_WITH_CONCERNS. Same `subagent_type: "poteto-agent"`.

```
description: Review <NN> (spec + quality)
subagent_type: poteto-agent
prompt: |
  You are reviewing one RamJob ticket implementation.
  This is a task gate, not milestone thermo.

  ## Requested

  Ticket: .scratch/ramjob/issues/<NN>-<slug>.md
  Brief: <BRIEF_FILE>
  Global constraints: <bullet list from SPEC / milestone>

  ## Claims

  Implementer report: <REPORT_FILE>

  ## Diff

  Base: <BASE_SHA or working-tree note>
  Head: <HEAD_SHA or working-tree note>
  Diff file if present: <DIFF_FILE>

  Read the diff once. Do not mutate the tree. Do not re-run full suites unless a specific doubt needs one focused check.

  ## Verdicts required

  1. Spec compliance. Matches acceptance criteria. Nothing extra that changes product shape.
  2. Code quality. Clear, tested where required, no YAGNI violations in-scope.

  ## Severity

  Critical / Important must block. Nit is optional.

  ## Output

  - Spec: PASS | FAIL
  - Quality: APPROVED | CHANGES_REQUIRED
  - Findings list with severity
  - Required fixes (if any)
```

---

## Fix loop Task

On CHANGES_REQUIRED with Critical or Important findings.

```
description: Fix <NN> review findings
subagent_type: poteto-agent
prompt: |
  Fix Critical and Important findings for ticket <NN>.
  Findings file or list: <FINDINGS>
  Ticket: .scratch/ramjob/issues/<NN>-<slug>.md
  Same boundaries as the implementer brief.
  Do not commit unless user approved commits this session.
  Re-verify the ticket Verify section. Report DONE or BLOCKED.
```

Then re-dispatch the task reviewer.
