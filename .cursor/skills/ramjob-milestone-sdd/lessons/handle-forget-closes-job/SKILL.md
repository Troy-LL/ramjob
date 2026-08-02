# handle forget closes job

## Leading word

handle-forget-closes-job

## When

Wrapping a store-owned Win32 `HANDLE` in a RAII type whose `Drop` calls `CloseHandle`, then using `mem::forget` only on the Ok path.

## Failure mode

`?` on apply/assign drops the temporary wrapper and closes the **live** job still held by the store → dangling HANDLE / use-after-close. M4 thermo Critical C1.

## Do this

1. Pass `&JobHandle` (or a non-owning view) into hooks; never build a second owning wrapper for the same HANDLE.
2. Add a unit test that forces hooks to `Err` and asserts the store job remains usable afterward.

## Done when

- No `mem::forget` on Job Object handles.
- Err-path tests cover set_limit and assign.

## Anti-pattern

```rust
let wrapper = JobHandle(store_handle);
hooks.apply(&wrapper)?;
mem::forget(wrapper);
```
