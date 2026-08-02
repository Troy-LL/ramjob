# Brief — ticket 66 M6 thermo fixes

## Ticket

`.scratch/ramjob/issues/66-m6-thermo-fixes.md`
Thermo: `.superpowers/sdd/m6-thermo-review.md` (C1, I1–I7)
Branch: `milestone/m6-shippable`

## Fix order (from thermo)

1. C1 pagefile MB→bytes
2. I1 touch_observed_groups write rate
3. I2/I3 autostart/HKCU failure handling
4. I4/I5/I6/I7

## Job

1. Fix all Critical + Important; leave Nits optional
2. Env: `. .\scripts\dev-env.ps1`; `$env:CARGO_TARGET_DIR="$env:USERPROFILE\ramjob-target"`
3. Verify tests/build
4. **Commit approved:** `fix(m6): thermo C1/I1–I7 shippable judo (task 7)`
5. Update progress.md; write `.superpowers/sdd/task-66-report.md` with evidence table

## Boundaries

No product-shape pivots. No new milestone scope. Own the files named in the thermo review.

## Report

DONE | DONE_WITH_CONCERNS | BLOCKED + SHA
