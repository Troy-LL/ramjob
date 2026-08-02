# List group footprint

List shows each application group that holds at least 50 MiB of private working-set footprint, with member count and human-readable GF, sorted largest first.

## Sub-features

- `list-default` runs when the user invokes `ramjob` with no subcommand or `ramjob list`.
- `list-floor` hides groups below the 50 MiB GF floor.
- `list-sort` orders remaining rows by GF descending.

## How to get to it (user POV)

- Run `ramjob` or `ramjob list` in a terminal from a built CLI.
- From source: `cargo run -p ramjob-cli -- list` after `dev-env.ps1`.

## Driving it with verify-ramjob

Preconditions:

- `helpers\verify-ramjob.ps1 doctor` exits 0.
- Evidence directory created for this run.

- **Enumerate.** Run `.\.cursor\skills\verify-ramjob\helpers\verify-ramjob.ps1 list -EvidenceDir <evidence>`. Exit code `0`.
- **Observe rows.** Open `<evidence>\list.stdout.txt`. Every non-empty line is `group_key\tmember_count\thuman_gf` (three tab-separated fields). At least one row is present on a normal desktop session.
- **Proof.** `<evidence>\meta.txt` records feature `list-gf` and exit `0`. Retain `list.stdout.txt` after cleanup.

## Gotchas

- Empty output can occur on a nearly idle machine with no group ≥ 50 MiB — treat as environmental, not a product pass; re-run with typical apps open or mark inconclusive.
- Group keys are path or `image:` prefixes, not friendly display names.
- Do not use mocked process tables; this path must call live NtQSI via the real binary.
