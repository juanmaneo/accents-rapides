// Checks the C ABI of the Rust staticlib as C++ sees it (accents_rapides.hpp):
// layout, flags, key names, sessions and string ownership. Links the staticlib only,
// not Fcitx5. Run by `cargo xtask fcitx5 test` with a fresh configuration,
// hence the default mappings, under AddressSanitizer and UBSan.

#include "accents_rapides.hpp"
#include "check.hpp"

#include <cstdint>
#include <cstdio>
#include <print>
#include <source_location>
#include <string_view>

namespace {

using namespace accents_rapides;

/// C strings are equal, NULL only to NULL.
constexpr bool same(const char *a, const char *b) noexcept {
    return a == nullptr || b == nullptr ? a == b : std::string_view{a} == b;
}

static_assert(same(nullptr, nullptr) && same("ä", "ä"));
static_assert(!same("a", nullptr) && !same(nullptr, "a") && !same("a", "b"));

constexpr std::string_view show(const char *text) noexcept {
    return text == nullptr ? "NULL" : text;
}

/// Check every field of `action`; NULL means no text.
void expect(const FcitxAccentsRapidesAction &action, const char *commit, const char *preedit,
            bool forward, std::source_location where = std::source_location::current()) {
    check(same(action.commit, commit), "commit", where);
    check(same(action.preedit, preedit), "preedit", where);
    check(action.forward == forward, "forward", where);
    if (!same(action.commit, commit) || !same(action.preedit, preedit) ||
        action.forward != forward) {
        std::println(stderr, "  got commit={} preedit={} forward={}", show(action.commit),
                     show(action.preedit), action.forward);
    }
}

// Physical key codes, as distinct values.
constexpr std::uint32_t kA = 38;
constexpr std::uint32_t kO = 32;
constexpr std::uint32_t kC = 54;
constexpr std::uint32_t kX = 53;
constexpr std::uint32_t kSpace = 65;
constexpr std::uint32_t kShift = 50;

/// The addon and the gesture state of one window.
struct Window {
    const Core &core;
    Session session{fcitx_accents_rapides_session_create()};
};

/// Send a key from a program named "test". `key` must be backed by a string
/// literal (as `key_name` constants are), so that it is NUL-terminated.
FcitxAccentsRapidesAction send(const Window &w, std::string_view key, std::uint32_t code,
                               std::uint32_t flags = 0) {
    return fcitx_accents_rapides_key_event(w.core.get(), w.session.get(), "test", key.data(),
                                           code, flags);
}

FcitxAccentsRapidesAction release(const Window &w, std::string_view key, std::uint32_t code) {
    return send(w, key, code, key_flag::release);
}

FcitxAccentsRapidesAction reset(const Window &w) {
    return fcitx_accents_rapides_reset(w.core.get(), w.session.get());
}

void gestures(const Window &w) {
    // A mapped key shows as preedit; the leader commits its only variant;
    // the release of the committed key is swallowed.
    expect(send(w, "a", kA), nullptr, "a", false);
    expect(send(w, key_name::space, kSpace), "ä", nullptr, false);
    expect(release(w, "a", kA), nullptr, nullptr, false);
    static_cast<void>(release(w, key_name::space, kSpace));

    // Unmapped keys and modifiers go through.
    expect(send(w, "x", kX), nullptr, nullptr, true);
    static_cast<void>(release(w, "x", kX));
    expect(send(w, "", kShift, key_flag::shift | key_flag::modifier), nullptr, nullptr, true);

    // Shift selects the uppercase mapping.
    expect(send(w, "A", kA, key_flag::shift), nullptr, "A", false);
    expect(send(w, key_name::space, kSpace, key_flag::shift), "Ä", nullptr, false);
    static_cast<void>(release(w, "A", kA));
    static_cast<void>(release(w, key_name::space, kSpace));
    static_cast<void>(release(w, "", kShift));
}

void shortcuts(const Window &w) {
    // A shortcut commits the pending key and goes through.
    for (const std::uint32_t modifier : {key_flag::ctrl, key_flag::alt, key_flag::super}) {
        expect(send(w, "o", kO), nullptr, "o", false);
        expect(send(w, "c", kC, modifier), "o", nullptr, true);
        static_cast<void>(release(w, "c", kC));
        static_cast<void>(release(w, "o", kO));
    }
}

void windows(const Core &core) {
    // A key held in one window is not held in another, where the leader is
    // an ordinary key and there is nothing to reset.
    const Window first{core};
    const Window second{core};
    expect(send(first, "o", kO), nullptr, "o", false);
    expect(send(second, key_name::space, kSpace), nullptr, nullptr, true);
    expect(reset(second), nullptr, nullptr, false);
    expect(send(first, key_name::space, kSpace), "ö", nullptr, false);
}

void resetAndReload(const Window &w) {
    // Reset hands the pending key back to the caller, once.
    expect(send(w, "o", kO), nullptr, "o", false);
    expect(reset(w), "o", nullptr, false);
    expect(reset(w), nullptr, nullptr, false);
    check(fcitx_accents_rapides_refresh(w.core.get()), "refresh");
    check(fcitx_accents_rapides_reload(w.core.get()), "reload");
}

void nullArguments(const Core &core) {
    // Without an addon or a session every call fails safe.
    const Core none;
    expect(send(Window{none}, "a", kA), nullptr, nullptr, true);
    const Window sessionless{core, nullptr};
    expect(send(sessionless, "a", kA), nullptr, nullptr, true);
    expect(reset(sessionless), nullptr, nullptr, true);
    expect(reset(Window{none}), nullptr, nullptr, true);
    check(!fcitx_accents_rapides_refresh(nullptr), "refresh(NULL)");
    check(!fcitx_accents_rapides_reload(nullptr), "reload(NULL)");
    fcitx_accents_rapides_destroy(nullptr);
    fcitx_accents_rapides_session_destroy(nullptr);
}

} // namespace

int main() {
    const Core core{fcitx_accents_rapides_create()};
    check(core != nullptr, "create");
    if (core) {
        const Window window{core};
        gestures(window);
        shortcuts(window);
        windows(core);
        resetAndReload(window);
    }
    nullArguments(core);
    return finish("abi_test");
}
