# C idiomatic rules

1. **Target the modern dialect** -- Compile with -std=c23 (or at least c17) and enable the full warning set -Wall -Wextra -Wpedantic plus UBSan/ASan for debug builds. New C23 niceties (binary literals, digit separators, _BitInt, decimal floats, [[nodiscard]], [[deprecated]], empty initializer ={}) are all fair-game.

2. **Prefer the standard headers** -- Always include <stdint.h>, <stdbool.h>, <stddef.h> etc. for fixed-width types, bool, and size_t; never roll your own.

3. **Const-correctness everywhere** -- Mark inputs const, pointers that never escape static, and use restrict for non-overlapping buffers to help optimisers.

4. **Functions over macros** -- Replace function-like macros with static inline or _Generic dispatch; reserve macros for compile-time switches and header guards (#pragma once).

5. **Initialise, don't assign** -- Use designated initialisers (struct Foo f = { .x = 1, .y = 0 };) and compound literals instead of scattered memset.

6. **Memory ownership is explicit** -- Pair every malloc/calloc with a free in the same control path; document ownership transfer in the API comment. Enable ASan for leaks.

7. **Error-first returns** -- Return negative error codes or an enum status; set errno only for true POSIX-level errors; never exit() from a library.

8. **No undefined behaviour** -- Avoid pointer arithmetic past array bounds, signed-integer overflow, and unchecked shifts; compile with UBSan and treat warnings as errors.

9. **Tidy headers** -- Keep each public API in its own <foo.h> with clear pre-conditions, post-conditions, and zero compile-time surprises.

10. **Borrow from MISRA C 2023** -- Where safety matters, adopt MISRA rules (no recursion in critical code, forbid dynamic memory after init, limit goto, etc.).
