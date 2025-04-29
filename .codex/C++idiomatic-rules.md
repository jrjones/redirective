### C++ idiomatic rules (C++23-ish)
(C) Copyright 2025, [Joseph R. Jones](https://jrj.org)
Licensed under MIT License

1. **Build for C++23** -- Set the target (cxx_std_23 in CMake) and turn on warnings & sanitizers. Embrace new features: modules, std::expected, std::print, formatting ranges, and UTF-8 as the default source encoding.

2. **Follow the C++ Core Guidelines** -- They capture RAII, type- and resource-safety, naming, and lifetime rules; clang-tidy has cppcoreguidelines-* checks to enforce them.

3. **Use modules, not mega-headers** -- Export libraries with export module foo; and import import std; or import std.compat; instead of #include <vector>.

4. **RAII and smart pointers only** -- Own with std::unique_ptr, share with std::shared_ptr, observe with raw or std::span; never call new/delete directly. std::experimental::unique_resource is handy for odd handles.

5. **Const-correctness, constexpr, consteval** -- Make everything immutable by default; push work to compile-time where sensible; prefer range-based for and algorithms from <ranges>.

6. **Model with strong types** -- Use enum class, tag-types, and std::chrono/std::string_view instead of plain int/char*; employ [[nodiscard]] to prevent ignored results.

7. **Concepts before typename** -- Express requirements (Sortable, std::ranges::input_range) so diagnostics stay readable and overload sets stay sane.

8. **Exceptions or std::expected** -- Pick one strategy per subsystem; never return raw error codes. Clean up in destructors or std::scope_exit.

9. **Concurrency via the STL** -- Prefer std::jthread, std::stop_token, std::atomic and high-level parallel algorithms; avoid manual thread management.

10. **Static analysis & sanitizers** -- Wire clang-tidy (modernize-*, cppcoreguidelines-*) and UBSan/ASan into CI, and keep the build _warning-clean_ (-Werror).
