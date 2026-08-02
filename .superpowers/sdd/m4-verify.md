# M4 Job Object backstop — hog verify (Task 5)

Run: 2026-08-03 · Branch `milestone/m4-job-backstop` · Ticket 44

## Environment

```powershell
. .\scripts\dev-env.ps1
$env:CARGO_TARGET_DIR="$env:USERPROFILE\ramjob-target"
```

## Commands + results

### 1. Build hog harness

```text
cargo build -p ramjob-hog
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.14s
```

### 2. `cargo test -p ramjob-core --lib`

```text
running 113 tests
test runtime::tests::tick_with_groups_soft_trim_follow_backstop_arms_mock_store ... ok
test job_backstop::tests::drop_closes_jobs_without_terminate ... ok
...
test result: ok. 111 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out
```

Key regression (task-43 Important): `tick_with_groups_soft_trim_follow_backstop_arms_mock_store` exercises the live tick path — three stubbed ineffective SoftTrims spaced past `TRIM_RATE_LIMIT`, `observe_post_trim` returns `follow=Backstop`, mock `JobBackstopStore` arms with `BACKSTOP arm limit=` diagnostic. Does **not** call `arm_backstop_if_ready_for_test`.

### 3. `cargo test -p ramjob-core --test m4_backstop`

```text
running 4 tests
test pack_never_sets_kill_on_job_close ... ok
test mock_store_arm_assign_limit_and_drop ... ok
test runtime_pressure_ticks_sample_without_backstop_arm ... ok
test live_hog_survives_backstop_store_drop ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured
```

| Test | Proves |
|---|---|
| `pack_never_sets_kill_on_job_close` | `KILL_ON_JOB_CLOSE` / `BREAKAWAY_OK` never set in packed limits |
| `mock_store_arm_assign_limit_and_drop` | Mock assign + limit; Drop closes via `close_job` (unit: `drop_closes_jobs_without_terminate`) |
| `runtime_pressure_ticks_sample_without_backstop_arm` | PRESSURE-only ticks sample commit_ratio without arming |
| `live_hog_survives_backstop_store_drop` | Win32 assign + drop store → hog still alive (`KILL_ON_JOB_CLOSE` off) |

### 4. `cargo test -p ramjob-core` (full crate)

Unit + `m4_backstop` green. `m2_integration` may fail on hosts with **Smart App Control** blocking fresh test binaries (`os error 4551`); not a backstop regression — re-run after unblock or use `ramjob-target` per `windows-smart-app-control-cargo` lesson.

## Limitations

- **Tick-path regression** uses injectable trim stub (`set_trim_measurement_stub`) in unit tests only — avoids flaky 60s+ live trim loops. Integration file cites the runtime test by name.
- **Live hog assign** may skip with diagnostic when the test runner is already inside a non-nestable job (CI/agent nested job).
- **Panel UI** unchanged (task 6 at `67726f0`).

## Honest summary

- Arm path: proven via `tick_with_groups` + stub trim → follow `Backstop` → mock store + diagnostic.
- Drop survival: proven via mock drop test + live hog Win32 test on this machine.
- Forbidden flags: proven via pack + unit tests.
