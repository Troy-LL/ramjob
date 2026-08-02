# 66 M6 thermo fixes

**Milestone.** M6

**What to build.** Fix every Critical/Important finding from `.superpowers/sdd/m6-thermo-review.md`.

**Blocked by.** Thermo complete (this ticket).

**Status.** done

## Acceptance criteria

- [x] **C1** Pagefile size: registry MB converted correctly; §5.4 Small fires on registry path; tests use real units
- [x] **I1** `touch_observed_groups` must not rewrite config ~1 Hz — debounce/day bucket or dirty-only save
- [x] **I2** HKCU sync failure on startup must not hard-fail app start — diagnose and continue
- [x] **I3** Autostart toggle: HKCU sync failure must not leave tray/config inconsistent (revert or sync-before-save)
- [x] **I4** Single note builder for diagnostics + panel (no dual string tables)
- [x] **I5** User-set caps mark `pinned = true` (or equivalent) so prune cannot drop them
- [x] **I6** Close elevation-probe token handle
- [x] **I7** Share registry wide/RegKey helpers between autostart and preflight (or thin common module)
- [x] Re-verify: `cargo test -p ramjob-core` + `cargo test -p ramjob-app` + `cargo build -p ramjob-app`

## Verify

Commands above green; thermo items C1 + I1–I7 closed in report with file:line evidence.
