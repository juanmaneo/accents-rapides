// C ABI of the accents-rapides core, exported by the Rust staticlib of
// crates/fcitx5 (src/addon.rs and src/lib.rs). Keep both sides in sync;
// abi_test.cpp checks them against each other.
#pragma once

#include <algorithm>
#include <array>
#include <bit>
#include <cstdint>
#include <functional>
#include <memory>
#include <string_view>
#include <type_traits>

extern "C" {

/// Opaque addon state owned by the Rust side.
struct FcitxAccentsRapides;
/// Opaque gesture state of one window (input context), owned by the Rust side.
struct FcitxAccentsRapidesSession;

/// What to do with a key event. NULL strings mean none; the others are owned
/// by the addon and stay valid until its next call.
struct FcitxAccentsRapidesAction {
    const char *commit;
    const char *preedit;
    bool forward;
};

// A Rust panic cannot cross `extern "C"`: it aborts. Hence `noexcept`.

/// Load the configuration and create the addon state; NULL on failure.
FcitxAccentsRapides *fcitx_accents_rapides_create() noexcept;
/// Destroy an addon (NULL is a no-op).
void fcitx_accents_rapides_destroy(FcitxAccentsRapides *addon) noexcept;
/// Create the gesture state of one window.
FcitxAccentsRapidesSession *fcitx_accents_rapides_session_create() noexcept;
/// Destroy a session (NULL is a no-op).
void fcitx_accents_rapides_session_destroy(FcitxAccentsRapidesSession *session) noexcept;
/// Handle a key event sent to `program`, in the window of `session`. `key` is
/// what the key types or its `key_name`, `flags` a combination of `key_flag`.
/// NULL or invalid arguments let the key through.
FcitxAccentsRapidesAction
fcitx_accents_rapides_key_event(FcitxAccentsRapides *addon, FcitxAccentsRapidesSession *session,
                                const char *program, const char *key, std::uint32_t code,
                                std::uint32_t flags) noexcept;
/// End the gesture of `session`, returning what it would commit.
FcitxAccentsRapidesAction fcitx_accents_rapides_reset(FcitxAccentsRapides *addon,
                                                      FcitxAccentsRapidesSession *session) noexcept;
/// Reload the configuration if its file changed since it was loaded; false
/// (keeping the current one) on failure.
bool fcitx_accents_rapides_refresh(FcitxAccentsRapides *addon) noexcept;
/// Reload the configuration; false (keeping the current one) on failure.
bool fcitx_accents_rapides_reload(FcitxAccentsRapides *addon) noexcept;

} // extern "C"

static_assert(std::is_standard_layout_v<FcitxAccentsRapidesAction> &&
                  std::is_trivially_copyable_v<FcitxAccentsRapidesAction>,
              "FcitxAccentsRapidesAction must stay a plain #[repr(C)] struct");

namespace accents_rapides {

/// Key event flags (`key_flags` in src/lib.rs).
namespace key_flag {
inline constexpr std::uint32_t release = 1U << 0;
inline constexpr std::uint32_t shift = 1U << 1;
inline constexpr std::uint32_t ctrl = 1U << 2;
inline constexpr std::uint32_t alt = 1U << 3;
inline constexpr std::uint32_t super = 1U << 4;
/// The key itself is a modifier.
inline constexpr std::uint32_t modifier = 1U << 5;

inline constexpr std::array all{release, shift, ctrl, alt, super, modifier};
static_assert(std::ranges::all_of(all, [](std::uint32_t f) { return std::has_single_bit(f); }),
              "each key flag is one bit");
static_assert(std::ranges::fold_left(all, 0U, std::bit_or{}) ==
                  (1U << all.size()) - 1U,
              "key flags are distinct and contiguous");
} // namespace key_flag

/// Names of the keys that the core knows by name (`key_name` in
/// crates/core/src/input.rs). Backed by string literals, so `.data()` is
/// NUL-terminated and can be passed to the C ABI as is.
namespace key_name {
inline constexpr std::string_view space = "space";
inline constexpr std::string_view left = "left";
inline constexpr std::string_view right = "right";
inline constexpr std::string_view up = "up";
inline constexpr std::string_view down = "down";
/// Left or right Alt.
inline constexpr std::string_view alt = "alt";
/// AltGr (ISO Level 3 Shift).
inline constexpr std::string_view altgr = "altgr";
} // namespace key_name

/// Frees a Rust-owned object with its C ABI `destroy` function.
template <auto Destroy> struct Deleter {
    void operator()(auto *object) const noexcept { Destroy(object); }
};

/// Owning handle on the addon state.
using Core = std::unique_ptr<FcitxAccentsRapides, Deleter<fcitx_accents_rapides_destroy>>;
/// Owning handle on the gesture state of one window.
using Session =
    std::unique_ptr<FcitxAccentsRapidesSession, Deleter<fcitx_accents_rapides_session_destroy>>;

} // namespace accents_rapides
