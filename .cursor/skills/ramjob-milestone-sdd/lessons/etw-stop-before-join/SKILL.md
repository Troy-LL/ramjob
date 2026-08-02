# etw-stop-before-join

## Leading word

etw-stop-before-join

## When

Starting a background ETW/WMI/COM consumer with a ready handshake, then timing out or failing open.

## Failure mode

`join()` the consumer while the session/trace is still running (no `ControlTraceW(STOP)` / shutdown flag first). Parent hangs; degrade path never runs. M5 thermo C1/C2.

## Do this

1. On failure/timeout: signal stop/shutdown **first**.
2. Then join (with optional timeout + abandon leak if uninterruptible).
3. Never use parent-thread `GetLastError` for a consumer syscall.

## Done when

- `select_discovery` cannot hang forever on open timeout.
- Unit/integration paths degrade to the next backend.

## Anti-pattern

```rust
// session still live
consumer.join(); // may block in ProcessTrace forever
drop(session);   // STOP never reached
```
