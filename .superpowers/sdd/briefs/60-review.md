# Brief — review ticket 60

## Ticket

`.scratch/ramjob/issues/60-config-autostart-prune.md`
Commit: `540ca15` on `milestone/m6-shippable`
Report: `.superpowers/sdd/task-60-report.md`

## Gates

1. Spec: autostart default false; pinned + last_seen_unix; prune_stale_groups 90-day; legacy parse; tests
2. Quality: YAGNI, no tray/HKCU creep

Note: implementer left `prune_stale_groups` unwired from `load_config_file`. Decide if that fails Spec (SPEC §8.3 “dropped on startup”) — if so CHANGES_REQUIRED; else Nit + later task.

## Output

Write `.superpowers/sdd/task-60-review.md`
Spec PASS|FAIL; Quality APPROVED|CHANGES_REQUIRED
No product code changes. No commit.
