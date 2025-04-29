# Universal Guidelines
(C) Copyright 2025, [Joseph R. Jones](https://jrj.org)
Licensed under MIT License

No matter the language, there are rules that you should always follow:

## 1 Know the Problem First
Start every file, function, and test by writing why it exists. A clear, human sentence at the top of the module prevents “clever” code that solves the wrong problem.

## 2 Separate Concerns Relentlessly
One module → one purpose. Keep public surfaces tiny; hide helpers behind internal, static, or anonymous namespaces. When a function grows past ~40 LOC or two distinct responsibilities, split it.

## 3 Model, Don’t Mangle
Represent real-world concepts with plain data types (structs, records, case classes). Push behaviour next to the data; avoid “anaemic” buckets of fields or god-objects.

## 4 Push State to the Edges
Treat core logic as pure functions. Delegate I/O, randomness, and clock access to thin boundary layers so they’re easy to stub in tests and change later.

## 5 Prefer Composition over Inheritance
Wire features together with small, composable pieces (functions, traits, protocols, interfaces). Inheritance is the last resort for true is-a relationships.

## 6 Dependency Injection, Not Global Reach
Pass explicit dependencies (as constructor args, parameters, or higher-order functions). Globals hurt testability and hide coupling.

## 7 Fail Fast, Fail Explicitly
Return typed errors (Result, expected, exceptions) instead of sentinel values. Log only actionable failures; crash early in development so problems surface before production.

## 8 Keep Functions Small and Deterministic
A good function:
	•	single logical path,
	•	≤ five parameters,
	•	no hidden side effects,
	•	unit-testable in milliseconds.

## 9 Automate Tests First
Write a failing unit or property test before plumbing. Keep fast tests in-process; push slower integration checks behind an opt-in flag and run them in CI.

## 10 Document at the Boundaries
Internal code should be self-evident; public APIs need concise doc comments, examples, and invariants. Architecture docs live beside the code and version with it.

## 11 Embrace Incremental Delivery
Ship thin vertical slices that walk through UI → service → DB. Refactor continuously; “big-bang” rewrites almost always slip and stall.

## 12 Design for Replaceability, Not Eternity
Assume each component will be rewritten in two years. Loose coupling, clear interfaces, and migration hooks beat speculative future-proofing.

## 13 Observe Everything
Add structured logs, metrics, and tracing in the platform-specific way (e.g., OpenTelemetry, os_signpost, tracing crate). Good telemetry turns unknowns into graphs, not guesswork.

## 14 Plan for Failure Modes Early
Identify the unhappy paths first: timeouts, partial outages, malformed input. Build retries, circuit breakers, and graceful degradation before scaling features.

## 15 Keep Learning & Challenge Assumptions
Treat guidelines—​including these—​as starting points. Review, measure, and refine as the project and your understanding evolve.
