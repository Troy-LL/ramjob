# Task 44 report — M4 hog / backstop verify

**Ticket:** 44-m4-verify  
**Branch:** `milestone/m4-job-backstop`  
**Commit:** `bcf9fb8` — `test(m4): job backstop verify + tick follow arm (task 5)`

## Done

- **`runtime::tests::tick_with_groups_soft_trim_follow_backstop_arms_mock_store`** — task-43 Important regression: full `tick_with_groups` path with stubbed ineffective SoftTrims (21s apart), `observe_post_trim` follow `Backstop`, mock `JobBackstopStore` armed with `BACKSTOP arm limit=` diagnostic. Not `arm_backstop_if_ready_for_test`.
- **`set_trim_measurement_stub`** — unit-test injectable trim hook on `measured_soft_trim` (no flaky live gate).
- **`crates/ramjob-core/tests/m4_backstop.rs`** — integration: forbidden flags, mock arm/drop, pressure sampling guard, live hog survives store Drop.
- **`.superpowers/sdd/m4-verify.md`** — commands + results.

## Tests

| Suite | Result |
|---|---|
| `cargo test -p ramjob-core --lib` | 111 passed, 2 ignored |
| `cargo test -p ramjob-core --test m4_backstop` | 4 passed |

## Notes

- Task-43 outer-only `Backstop` path still covered by `outer_backstop_action_arms_mock_job_store`; live ticks arm on **follow** after SoftTrim.
- Panel task 45 already at `67726f0` — no UI changes in this task.
