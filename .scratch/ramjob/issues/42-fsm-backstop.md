# 42 — FSM Backstop action

**Milestone:** M4 · Plan Task 3  
**Depends on:** 40

## Goal
Replace `RecordWouldBackstop` with real `FsmAction::Backstop` when opted in (`always_enforce`); opt-out keeps soft-stop / no hard arm.

## Verify
`cargo test -p ramjob-core fsm`
