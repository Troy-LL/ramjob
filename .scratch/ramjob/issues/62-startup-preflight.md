# 62 Startup preflight

**Milestone.** M6

**What to build.** Once per process start, run SPEC §5.4 environment preflight and surface a
structured `PreflightReport` for diagnostics and first-run UI.

**Blocked by.** None (parallel-safe with 60/61 if trees disjoint — prefer after 60 if sharing AppState types).

**Status.** ready-for-agent

## Acceptance criteria

- [ ] `preflight::run_once()` → pagefile presence/size note, total RAM, ≥32 GB dormancy note, privilege notes per §5.4
- [ ] Results push into diagnostics ring (or return report for app to push)
- [ ] Idempotent / once-per-process API documented
- [ ] Unit tests for report fields with mocked sysinfo where practical

## Verify

`cargo test` covering preflight; report non-empty on real host smoke in task report.
