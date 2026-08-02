# 64 First-run + preflight panel copy

**Milestone.** M6

**What to build.** Panel snapshot carries first-run / preflight hints; UI shows SPEC §7.3 one-liner
and high-RAM dormancy note when applicable.

**Blocked by.** 62

**Status.** ready-for-agent

## Acceptance criteria

- [ ] Snapshot includes first_run (no caps set) and preflight note fields
- [ ] UI shows explainer + dormancy note without a wizard
- [ ] Copy stays short; no new dashboard chrome

## Verify

`cargo build -p ramjob-app`; UI strings match SPEC §7.3 / §5.4 intent.
