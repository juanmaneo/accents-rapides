// The checks of the C++ tests: count failures and report where they happen,
// so that one run lists every failed check instead of stopping at the first.
#pragma once

#include <cstdio>
#include <cstdlib>
#include <print>
#include <source_location>
#include <string_view>

namespace accents_rapides {

inline int failures = 0;

inline void check(bool ok, std::string_view what,
                  std::source_location where = std::source_location::current()) {
    if (!ok) {
        ++failures;
        std::println(stderr, "{}:{}: check failed: {}", where.file_name(), where.line(), what);
    }
}

/// Report the outcome of the test `name`, and return its exit status.
inline int finish(std::string_view name) {
    if (failures != 0) {
        std::println(stderr, "{}: {} check(s) failed", name, failures);
        return EXIT_FAILURE;
    }
    std::println("{}: all checks passed", name);
    return EXIT_SUCCESS;
}

} // namespace accents_rapides
