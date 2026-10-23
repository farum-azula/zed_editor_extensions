# Bugs

Found by hand-testing; each one names the cause where it is known and the
fix to make. Delete a bug when it is fixed (the CHANGELOG of the extension
records the fix).

## Flutter Tools (`flutter-extension`)

- [ ] **No `Dart: debug` scenario for the plain Dart tasks.** The `dart run`
  tasks exist (`languages/dart/tasks.json`) and a `.zed/debug.json`
  scenario of the `Dart` adapter debugs a program (README, "Plain Dart"), but
  the Debug tab offers none for the tasks the way it offers `Flutter: debug
  on <platform>`: that is a second debug locator in `src/lib.rs` (Rust, WASM)
  that turns a `dart run ...` task into a `Dart` scenario with `program`.
