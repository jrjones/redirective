### **Executive cheat-sheet: 18 guardrails CodexCLI should obey for idiomatic Rust**
(C) Copyright 2025, [Joseph R. Jones](https://jrj.org)
Licensed under MIT License

_(all rules are distilled from The Rust Programming Language book; follow them in order)___

1. **Project scaffolding & tooling** -- Always start with cargo new/cargo init; keep code in src/, docs in README, examples in examples/; run cargo fmt, cargo clippy --deny warnings, and cargo test before you surface code.

2. **Ownership first** -- Every value has exactly one owner; moves transfer ownership, and a moved-from value must not be used. Call clone only when you really need a deep copy.

3. **Borrowing & lifetimes** -- Prefer shared references (&T) or exclusive references (&mut T) to moving; at any instant you may have _either_ many immutable _or_ one mutable reference. Add explicit lifetime parameters only when the compiler can't infer them.

4. **Use lightweight views** -- Pass &str, slices (&[T]) or iterator adaptors instead of owned String, Vec<T> or indexing loops whenever ownership isn't required.

5. **Smart-pointer etiquette** --

    - Box<T> for heap storage/recursive types,

    - Rc<T> for shared ownership in one thread,

    - Arc<T> + Mutex<T>/RwLock<T> for cross-thread shared state,

and avoid reference cycles (Rc<RefCell<T>>). Prefer channels or async tasks for coordination.

6. **Iterators over indices** -- Compose iter()/into_iter() with map, filter, fold, etc.; they compile to the same machine code as hand-written loops while staying clearer.

7. **Model with enums & exhaustive match** -- Keep match arms exhaustive; restrict a wildcard arm (_) to future-proofing, not to hide real variants.

8. **Traits & generics** -- Use traits to express behaviour; add trait bounds (T: Display) and where clauses for readability; default to static dispatch, resort to dyn Trait only when you need runtime polymorphism.

9. **Document as you code** -- Use /// doc comments on every public item and embed examples that compile (cargo test will run them).

10. **Testing discipline** -- Place unit tests in a #[cfg(test)] mod tests block in the same file; integration tests live in tests/. Run cargo test early and often.

11. **Error handling** -- Return Result<T,E> from any fallible function, propagate with ?, and reserve panic! for truly unrecoverable invariants. Never unwrap/expect in library code.

12. **CLI entry points** -- Let main return Result<(), Box<dyn error::Error>> (or your own error enum) and map errors to friendly messages there.

13. **Fearless concurrency** -- Prefer message-passing (mpsc channels) over shared state; guard shared data with Mutex<T>; types crossing thread boundaries must be Send + Sync. For I/O-bound tasks, use async/await with an executor.

14. **Minimise public surface** -- Default everything to private; expose only intentional APIs with pub/pub(crate) and re-export cleanly via pub use.

15. **Naming conventions** -- snake_case for functions, variables, modules; UpperCamelCase for structs, enums, traits; SCREAMING_SNAKE_CASE for constants; macro names are snake_case!.

16. **Macros with care** -- Reach for macro_rules! to remove repetition; keep macros small, hygienic, documented, and export them only when necessary.

17. **Unsafe is a last resort** -- Isolate unsafe blocks, state _why they're safe_ in comments, and wrap them in safe abstractions.

18. **Derive before you hand-roll** -- Use #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize, …)] to get correct, efficient impls automatically.

