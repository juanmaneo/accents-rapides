// Native Fcitx5 engine of accents-rapides.
//
// A thin shim: it translates each Fcitx5 key event for the Rust core
// (accents_rapides.hpp) and applies the action it returns. All the gesture
// logic lives in Rust; each input context (window) carries its own gesture
// state as an Fcitx5 property. Everything that can be is computed at compile
// time and checked with static_assert.

#include "accents_rapides.hpp"

#include <algorithm>
#include <array>
#include <cstdint>
#include <string>
#include <string_view>
#include <utility>

#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/log.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addoninstance.h>
#include <fcitx/addonmanager.h>
#include <fcitx/event.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>
#include <fcitx/text.h>

namespace accents_rapides {
namespace {

// --- Key translation (compile time) -----------------------------------------

struct NamedKey {
    fcitx::KeySym sym;
    std::string_view name;
};

/// Keys the core knows by name rather than by what they type.
constexpr std::array kNamedKeys{
    NamedKey{FcitxKey_space, key_name::space},
    NamedKey{FcitxKey_Left, key_name::left},
    NamedKey{FcitxKey_Right, key_name::right},
    NamedKey{FcitxKey_Up, key_name::up},
    NamedKey{FcitxKey_Down, key_name::down},
    NamedKey{FcitxKey_Alt_L, key_name::alt},
    NamedKey{FcitxKey_Alt_R, key_name::alt},
    NamedKey{FcitxKey_ISO_Level3_Shift, key_name::altgr},
};

static_assert(
    [] {
        auto keys = kNamedKeys;
        std::ranges::sort(keys, {}, &NamedKey::sym);
        return std::ranges::adjacent_find(keys, {}, &NamedKey::sym) == keys.end();
    }(),
    "a key symbol is named twice");

/// The `key_name` of `sym`, or empty if the core knows it by its text.
constexpr std::string_view keyName(fcitx::KeySym sym) noexcept {
    const auto it = std::ranges::find(kNamedKeys, sym, &NamedKey::sym);
    return it == kNamedKeys.end() ? std::string_view{} : it->name;
}

static_assert(keyName(FcitxKey_space) == key_name::space);
static_assert(keyName(FcitxKey_Alt_R) == key_name::alt);
static_assert(keyName(FcitxKey_ISO_Level3_Shift) == key_name::altgr);
static_assert(keyName(FcitxKey_a).empty());

/// Shift, Control, Caps Lock, Meta, Alt, Super and Hyper, as in the original
/// addon. AltGr (ISO_Level3_Shift) is outside the range on purpose: it can
/// be a leader.
constexpr bool isModifier(fcitx::KeySym sym) noexcept {
    return sym >= FcitxKey_Shift_L && sym <= FcitxKey_Hyper_R;
}

static_assert(isModifier(FcitxKey_Shift_L) && isModifier(FcitxKey_Alt_L) &&
              isModifier(FcitxKey_Super_L) && isModifier(FcitxKey_Hyper_R));
static_assert(!isModifier(FcitxKey_ISO_Level3_Shift) && !isModifier(FcitxKey_a) &&
              !isModifier(FcitxKey_space));

struct StateFlag {
    fcitx::KeyState state;
    std::uint32_t flag;
};

/// Fcitx5 modifier states and the `key_flag` each one maps to.
constexpr std::array kStateFlags{
    StateFlag{fcitx::KeyState::Shift, key_flag::shift},
    StateFlag{fcitx::KeyState::Ctrl, key_flag::ctrl},
    StateFlag{fcitx::KeyState::Alt, key_flag::alt},
    StateFlag{fcitx::KeyState::Super, key_flag::super},
};

/// The `key_flag`s of a key event. `states` is the raw integer of
/// `fcitx::KeyStates`, whose constructor from an integer is not constexpr.
constexpr std::uint32_t keyFlags(fcitx::KeySym sym, std::uint32_t states,
                                 bool release) noexcept {
    std::uint32_t flags = (release ? key_flag::release : 0U) |
                          (isModifier(sym) ? key_flag::modifier : 0U);
    for (const auto &[state, flag] : kStateFlags) {
        if ((states & std::to_underlying(state)) != 0U) {
            flags |= flag;
        }
    }
    return flags;
}

constexpr auto state(fcitx::KeyState s) noexcept { return std::to_underlying(s); }

static_assert(keyFlags(FcitxKey_a, 0U, false) == 0U);
static_assert(keyFlags(FcitxKey_a, 0U, true) == key_flag::release);
static_assert(keyFlags(FcitxKey_A, state(fcitx::KeyState::Shift), false) ==
              key_flag::shift);
static_assert(keyFlags(FcitxKey_c,
                       state(fcitx::KeyState::Ctrl) | state(fcitx::KeyState::Alt) |
                           state(fcitx::KeyState::Super),
                       false) == (key_flag::ctrl | key_flag::alt | key_flag::super));
static_assert(keyFlags(FcitxKey_Shift_L, 0U, true) ==
              (key_flag::release | key_flag::modifier));
// States the core does not care about are dropped.
static_assert(keyFlags(FcitxKey_a,
                       state(fcitx::KeyState::CapsLock) | state(fcitx::KeyState::NumLock),
                       false) == 0U);

// --- Fcitx5 glue (run time) --------------------------------------------------

/// Apply an action of the core to `ic`.
void apply(fcitx::InputContext &ic, const FcitxAccentsRapidesAction &action) {
    if (action.commit != nullptr) {
        ic.inputPanel().reset();
        ic.commitString(action.commit);
        ic.updatePreedit();
    }
    if (action.preedit != nullptr) {
        fcitx::Text preedit{action.preedit};
        preedit.setCursor(static_cast<int>(preedit.textLength()));
        ic.inputPanel().setClientPreedit(preedit);
        ic.updatePreedit();
    }
}

/// Log a failed reload; the current configuration stays.
void checkReload(bool reloaded) {
    if (!reloaded) {
        FCITX_ERROR() << "accents-rapides: cannot reload the configuration, "
                         "keeping the current one";
    }
}

/// The gesture state of one input context, created with it.
class SessionProperty final : public fcitx::InputContextProperty {
public:
    [[nodiscard]] FcitxAccentsRapidesSession *get() const noexcept { return session_.get(); }

private:
    Session session_{fcitx_accents_rapides_session_create()};
};

class Engine final : public fcitx::InputMethodEngineV2 {
public:
    Engine(fcitx::Instance &instance, Core core) : core_(std::move(core)) {
        instance.inputContextManager().registerProperty("accents-rapides-session", &sessions_);
    }

    void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) override {
        auto &ic = *event.inputContext();
        const fcitx::Key key = event.key();
        const fcitx::KeySym sym = key.sym();

        // Named keys need no allocation: their names are NUL-terminated literals.
        const std::string_view name = keyName(sym);
        const std::string typed = name.empty() ? fcitx::Key::keySymToUTF8(sym) : std::string{};
        const char *text = name.empty() ? typed.c_str() : name.data();

        // The modifiers come from the raw key: Fcitx5 normalizes Shift+A to
        // A, dropping the Shift that selects the uppercase mapping.
        const fcitx::Key &raw = event.rawKey();
        const auto action = fcitx_accents_rapides_key_event(
            core_.get(), session(ic), ic.program().c_str(), text,
            static_cast<std::uint32_t>(raw.code()),
            keyFlags(sym, raw.states().toInteger(), event.isRelease()));
        apply(ic, action);
        if (!action.forward) {
            event.filterAndAccept();
        }
    }

    /// Start the window clean, whatever happened before (crash, restart...),
    /// with the configuration as last saved.
    void activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
        static_cast<void>(fcitx_accents_rapides_reset(core_.get(), session(*event.inputContext())));
        checkReload(fcitx_accents_rapides_refresh(core_.get()));
    }

    /// On an input method switch the server does not commit the preedit by
    /// itself (it does on focus out), so the pending key is committed here.
    void deactivate(const fcitx::InputMethodEntry &,
                    fcitx::InputContextEvent &event) override {
        forget(*event.inputContext(),
               event.type() == fcitx::EventType::InputContextSwitchInputMethod);
    }

    /// The application reset its input (the caret moved...): the pending key
    /// is dropped.
    void reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override {
        forget(*event.inputContext(), false);
    }

    void reloadConfig() override { checkReload(fcitx_accents_rapides_reload(core_.get())); }

private:
    /// End the gesture of `ic` and clear its preedit, committing the pending
    /// key only if `commit`.
    void forget(fcitx::InputContext &ic, bool commit) {
        const auto action = fcitx_accents_rapides_reset(core_.get(), session(ic));
        ic.inputPanel().reset();
        if (commit && action.commit != nullptr) {
            ic.commitString(action.commit);
        }
        ic.updatePreedit();
    }
    FcitxAccentsRapidesSession *session(fcitx::InputContext &ic) {
        return ic.propertyFor(&sessions_)->get();
    }

    Core core_;
    // Declared last, so destroyed first: it unregisters itself and frees
    // every window's session.
    fcitx::SimpleInputContextPropertyFactory<SessionProperty> sessions_;
};

class Factory final : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        Core core{fcitx_accents_rapides_create()};
        if (!core) {
            FCITX_ERROR() << "accents-rapides: cannot load the configuration";
            return nullptr;
        }
        return new Engine(*manager->instance(), std::move(core));
    }
};

} // namespace
} // namespace accents_rapides

FCITX_ADDON_FACTORY(accents_rapides::Factory)
