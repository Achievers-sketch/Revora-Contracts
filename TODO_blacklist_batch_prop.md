# TODO: #477 Add proptest harness for blacklist add/remove state convergence

## Steps
- [ ] Rework `src/test_blacklist_batch_prop.rs` into a correct, compiling proptest harness.
  - [ ] Generate `add_vec` and `rem_vec` using `proptest::collection::vec(any::<u8>(), 0..50)`.
  - [ ] Map u8 values to deterministic Addresses via a fixed address pool (enables duplicates/overlaps).
  - [ ] Run the contract twice: original input order vs shuffled input order.
  - [ ] Assert final blacklist state equality (compare sorted address vecs).
  - [ ] Collect `bl_add` and `bl_rem` events only; normalize to `(kind, caller, investor)` and compare as a multiset via sorting.
  - [ ] Edge case: if both vectors are empty, assert no `bl_*` events were emitted (ignore setup events).
- [ ] Ensure the test compiles and is included in `cargo test` automatically.
- [ ] Run `cargo test --all`.
- [ ] If failures occur, fix compilation/runtime issues and re-run.

