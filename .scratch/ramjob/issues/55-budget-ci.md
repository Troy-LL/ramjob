# 55 — Budget CI harness

**Milestone:** M5 · Plan Task 6 · Depends: 54 preferred

## Goal
CI-ready assert for SPEC §6 idle ceilings: idle WS ≤ 25 MB (ceiling); document CPU if measurement is flaky.

## Acceptance
- Helper to sample RamJob process WS (and CPU if reliable)
- Test or script that fails when idle WS > 25 MB after short settle (or is ignored with clear reason on CI)
- `.superpowers/sdd/m5-verify.md` with commands + results
- Honest limitations (SAC, short sample noise)

## Verify
Documented commands in m5-verify.md pass or skip with rationale.
