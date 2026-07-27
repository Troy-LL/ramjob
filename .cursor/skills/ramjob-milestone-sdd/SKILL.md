---
name: ramjob-milestone-sdd
description: milestone gate for RamJob. Run the current SPEC milestone through SDD, end-of-milestone thermo, and lesson capture.
disable-model-invocation: true
---

# RamJob milestone gate

Controller skill for one SPEC milestone at a time. Product is **RamJob**. Stack is Rust + windows-rs for M0 CLI. Tauri comes later. Milestones M0 to M6 live in `SPEC.md` §10.

**Leading word.** milestone gate

## Hard rules

| Rule | Binding |
|------|---------|
| Before a plan | Brainstorm first; recommendation-first on options; grill-with-docs when OPENs remain; update SPEC (untruncated SOT); then writing-plans |
| Ticket scope | Only `.scratch/ramjob/issues/` tickets for the **current** milestone |
| Task dispatch | Every implement/review/fix `Task` uses `subagent_type: "poteto-agent"`; controller uses poteto-mode |
| Coding models | Weaker than parent; same family (Cursor parent → Cursor model; other parent → weaker same-family). Always pass explicit `model:` on coding Tasks |
| Thermo timing | Once per milestone after verify. `thermo-nuclear-code-quality-review-subagent`. Never per task |
| Commits | One commit per SDD task/phase. Message references the ticket |
| SDD ledger | `.superpowers/sdd/progress.md` |
| Parallel writes | Never two implementers on the same crate or tree |
| Parallel OK | Explore/research, arena designs, truly disjoint streams only |
| SPEC / docs | Never truncate SOT sections; fold decisions into SPEC when they land |

## Steps

### 1. Scope the milestone gate

1. Read `SPEC.md` §10. Name the current milestone (M0 to M6).
2. List tickets under `.scratch/ramjob/issues/` that belong to that milestone only.
3. Refuse work on later-milestone tickets.
4. Init or update `.superpowers/sdd/progress.md` for this milestone.

**Done when.** Current milestone id is written in the ledger. Ticket list matches that milestone only. Zero later-milestone tickets are in the work queue.

Ticket shape. Load [refs/ticket-template.md](refs/ticket-template.md) when authoring or checking tickets.

### 2. Per-task SDD loop

For each ready ticket (blockers done), in blockers-first order.

1. Write a task brief from [refs/implementer-brief.md](refs/implementer-brief.md).
2. Dispatch implementer via `Task` with `subagent_type: "poteto-agent"`.
3. On DONE or DONE_WITH_CONCERNS, dispatch task reviewer via `Task` with `subagent_type: "poteto-agent"` (reviewer section of the same ref).
4. On Critical/Important findings, dispatch fix via `Task` with `subagent_type: "poteto-agent"`, then re-review.
5. Mark the ticket complete in the ledger only after reviewer approval.
6. Do **not** dispatch thermo in this step.

**Done when.** Every current-milestone ticket has reviewer status approved (or explicitly deferred with human BLOCKED). Ledger row per ticket shows implementer then review then optional fix then approved. No thermo Task was fired during this step.

### 3. Milestone thermo gate

Run only after step 2 is complete for the milestone.

1. Run the milestone verify command or checklist from the tickets / SPEC (real artifact, not vibes).
2. Dispatch `thermo-nuclear-code-quality-review-subagent` **once** using [refs/thermo-prompt.md](refs/thermo-prompt.md).
3. Fix every Critical and Important finding (still `poteto-agent` for fix Tasks).
4. Re-run milestone verify.
5. Record thermo outcome in the ledger.

**Done when.** Milestone verify passes after thermo fixes. Ledger shows one thermo dispatch for this milestone. Critical count is 0. Important count is 0 or explicitly waived by the human.

### 4. Lesson capture

Before opening the next milestone.

1. Scan approach failures from this milestone (wrong API assumption, parallel-write conflict, skipped gate, flaky harness protocol, GF double-count, and similar).
2. Classify each. One-off typo vs repeatable failure mode.
3. For each repeatable mode, prefer harness assert, clippy lint, or CI check when enforceable.
4. Otherwise author one narrow skill at `.cursor/skills/ramjob-milestone-sdd/lessons/<slug>/SKILL.md`.
5. Link it from [lessons/README.md](lessons/README.md).
6. One failure mode to one skill. Merge duplicates. No spam.

**Done when.** Lessons index lists every new lesson skill (or states "none"). Each repeatable failure either has a structural guard or exactly one lesson skill. No duplicate lesson for the same mode.

### 5. Human stop on M1 §9.3

After M1 milestone thermo (step 3) and lesson capture (step 4).

1. Read SPEC §9.2 outcome (Pass / Marginal / Fail) from harness numbers.
2. If Marginal or Fail, stop. Present §9.3 pivot options. Wait for an explicit human decision.
3. Never silently switch to backstop-primary or rewrite product shape.
4. Do not ticket M2 until Pass, or until the human records a §9.3 decision.
5. Do not ticket M3 to M6 until that M1 gate decision exists.

**Done when.** Either (a) M1 Pass is recorded and M2 may be ticketed, or (b) human §9.3 decision is recorded in the ledger / SPEC and downstream ticket scope matches that decision. No M3 to M6 issue files exist until (a) or (b).

## Parallelism quick check

Before dispatching a second implementer.

- [ ] Trees and crates do not overlap
- [ ] Neither task mutates shared types the other is editing
- [ ] Both are past any shared scaffold blocker

If any box is unchecked, run them serial.

## Refs

| When | Load |
|------|------|
| Authoring or auditing tickets | [refs/ticket-template.md](refs/ticket-template.md) |
| Dispatching implementer or task reviewer | [refs/implementer-brief.md](refs/implementer-brief.md) |
| End-of-milestone thermo | [refs/thermo-prompt.md](refs/thermo-prompt.md) |
| Lesson index | [lessons/README.md](lessons/README.md) |
