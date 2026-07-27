# RamJob ticket template

Path. `.scratch/ramjob/issues/<NN>-<slug>.md`

Number from `01` in dependency order. Blockers first. One ticket per file. One milestone per batch.

```markdown
# <NN> <Ticket title>

**Milestone.** M<n> (from SPEC.md §10)

**What to build.** End-to-end behaviour this ticket makes work, from the user's perspective.
Not a layer-by-layer implementation list.

**Blocked by.** Ticket numbers/titles that gate this one, or `None`.

**Status.** ready-for-agent

## Acceptance criteria

- [ ] Criterion 1 (checkable)
- [ ] Criterion 2 (checkable)

## Verify

How a human or agent proves the criteria on a real artifact (CLI output, harness number, grouping dump).
```

## Rules

- Tracer bullet. Narrow complete path. Demoable or verifiable alone.
- Sized for one fresh implementer context.
- Avoid stale file paths and code dumps unless a prototype encoded a decision (type shape, FSM).
- Stack context for M0. Rust + windows-rs CLI. Tauri is out of scope until later milestones.
- Do not author M3 to M6 tickets until M1 §9.3 is decided.
