# Compression gate

Gate soft-trims a target image (or PID), measures reclaim yield (`Ry_bench` / `Ry_live`), and prints a Pass / Marginal / Fail / n/a classification plus a markdown report.

## Sub-features

- `gate-image` targets processes by image name (`--image`).
- `gate-pid` targets a single PID (`--pid`).
- `gate-out` writes the markdown report to `--out` (default under `.superpowers/sdd/` if omitted — verify always passes an evidence path).

## How to get to it (user POV)

- Run `ramjob gate --image <name>` or `ramjob gate --pid <n>` while the target is resident.
- Typical lab path: start `ramjob-hog`, then gate against `ramjob-hog`.

## Driving it with verify-ramjob

Preconditions:

- Doctor OK.
- Enough free RAM to allocate the hog (`-Mb`, default 64).

- **Drive.** Run `.\.cursor\skills\verify-ramjob\helpers\verify-ramjob.ps1 gate -EvidenceDir <evidence>`. Exit code `0`.
- **Observe.** `<evidence>\gate.stdout.txt` contains a `Classification:` line. `<evidence>\gate-out.md` exists and is non-empty.
- **Proof.** Classification may be Pass, Marginal, Fail, or n/a depending on machine pressure — proof is that the gate completed and both artifacts exist with a classification line, not that the grade is Pass.
- **Cleanup.** `helpers\verify-ramjob.ps1 cleanup -EvidenceDir <evidence>` stops any leftover hog PID.

## Gotchas

- Mutually exclusive: do not pass both `--image` and `--pid`.
- Hog must still be alive when gate samples; helper holds hog for `--hold-secs` and waits up to 15s.
- Trimming real user apps is invasive — prefer `ramjob-hog` for verify runs.
- Smart App Control can block a fresh `ramjob-hog` build; fix target dir before blaming gate logic.
