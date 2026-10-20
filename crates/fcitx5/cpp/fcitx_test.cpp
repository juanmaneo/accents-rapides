// Integration test: the addon as Fcitx5 loads it, inside a real
// fcitx::Instance, driven by Fcitx5's testfrontend addon the way applications
// drive it. Run by `cargo xtask fcitx5 test` on the installed file layout:
//
//   fcitx_test <addon dir> <data dir> <config file>
//
// with a fresh configuration, hence the default mappings. Only the testim and
// testfrontend addons are enabled besides ours, and only the given
// directories are searched, so whatever is installed system-wide stays out.

#include "check.hpp"

#include <array>
#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <format>
#include <fstream>
#include <iterator>
#include <map>
#include <memory>
#include <print>
#include <source_location>
#include <string>
#include <string_view>
#include <utility>

#include <fcitx-module/testfrontend/testfrontend_public.h>
#include <fcitx-utils/eventdispatcher.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/testing.h>
#include <fcitx/addonmanager.h>
#include <fcitx/event.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputmethodgroup.h>
#include <fcitx/inputmethodmanager.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>

namespace {

using namespace accents_rapides;

constexpr std::string_view kAddon = "accents-rapides";
/// The keyboard layout input method of the testim addon.
constexpr std::string_view kKeyboard = "keyboard-us";
/// The program owning the test windows.
constexpr std::string_view kProgram = "fcitx-test";

/// A key of the keyboard: what it types, and its physical code.
struct PhysicalKey {
    fcitx::KeySym sym;
    int code;
};

constexpr PhysicalKey kA{FcitxKey_a, 38};
constexpr PhysicalKey kShiftA{FcitxKey_A, 38};
constexpr PhysicalKey kS{FcitxKey_s, 39};
constexpr PhysicalKey kO{FcitxKey_o, 32};
constexpr PhysicalKey kT{FcitxKey_t, 28};
constexpr PhysicalKey kX{FcitxKey_x, 53};
/// The default leader.
constexpr PhysicalKey kAlt{FcitxKey_Alt_L, 64};
constexpr PhysicalKey kShift{FcitxKey_Shift_L, 50};

static_assert(kA.code == kShiftA.code, "Shift changes the symbol, not the key");
static_assert(kA.code != kS.code && kS.code != kO.code && kO.code != kT.code &&
              kT.code != kX.code && kX.code != kAlt.code);

fcitx::Key key(PhysicalKey physical, fcitx::KeyStates states = fcitx::KeyStates{}) {
    return fcitx::Key{physical.sym, states, physical.code};
}

/// What a window shows after an event: whether the input method took the
/// key (the application does not see it), what got committed, the preedit.
struct Outcome {
    bool taken = false;
    std::string commit;
    std::string preedit;

    bool operator==(const Outcome &) const = default;
};

void expect(const Outcome &got, const Outcome &want,
            std::source_location where = std::source_location::current()) {
    check(got == want, "outcome", where);
    if (got != want) {
        std::println(stderr, "  got taken={} commit=\"{}\" preedit=\"{}\"", got.taken, got.commit,
                     got.preedit);
        std::println(stderr, "  want taken={} commit=\"{}\" preedit=\"{}\"", want.taken,
                     want.commit, want.preedit);
    }
}

/// The running server: Fcitx5, the test frontend, and the text committed to
/// each window.
class Server {
public:
    Server(fcitx::Instance &instance, fcitx::AddonInstance &frontend)
        : instance_(instance), frontend_(frontend),
          watcher_(instance.watchEvent(
              fcitx::EventType::InputContextCommitString, fcitx::EventWatcherPhase::Default,
              [this](fcitx::Event &event) {
                  const auto &commit = static_cast<fcitx::CommitStringEvent &>(event);
                  commits_[commit.inputContext()] += commit.text();
              })) {}

    fcitx::Instance &instance() const noexcept { return instance_; }
    fcitx::AddonInstance &frontend() const noexcept { return frontend_; }

    /// What was committed to `ic` since the last call.
    std::string takeCommits(const fcitx::InputContext &ic) {
        return std::exchange(commits_[&ic], {});
    }

private:
    fcitx::Instance &instance_;
    fcitx::AddonInstance &frontend_;
    std::map<const fcitx::InputContext *, std::string> commits_;
    std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> watcher_;
};

/// An application window: a focused input context of the test frontend,
/// using our input method.
class Window {
public:
    explicit Window(Server &server)
        : server_(server), uuid_(server.frontend().call<fcitx::ITestFrontend::createInputContext>(
                               std::string{kProgram})),
          ic_(*server.instance().inputContextManager().findByUUID(uuid_)) {
        ic_.focusIn();
        switchTo(kAddon);
    }

    Window(const Window &) = delete;
    Window &operator=(const Window &) = delete;

    ~Window() { server_.frontend().call<fcitx::ITestFrontend::destroyInputContext>(uuid_); }

    Outcome press(PhysicalKey physical, fcitx::KeyStates states = fcitx::KeyStates{}) {
        return send(key(physical, states), false);
    }
    Outcome release(PhysicalKey physical, fcitx::KeyStates states = fcitx::KeyStates{}) {
        return send(key(physical, states), true);
    }

    Outcome switchTo(std::string_view inputMethod) {
        server_.instance().setCurrentInputMethod(&ic_, std::string{inputMethod}, true);
        return outcome(false);
    }

    Outcome focusOut() {
        ic_.focusOut();
        return outcome(false);
    }
    Outcome focusIn() {
        ic_.focusIn();
        return outcome(false);
    }
    Outcome reset() {
        ic_.reset();
        return outcome(false);
    }

private:
    Outcome send(const fcitx::Key &k, bool isRelease) {
        return outcome(
            server_.frontend().call<fcitx::ITestFrontend::sendKeyEvent>(uuid_, k, isRelease));
    }

    Outcome outcome(bool taken) {
        return {taken, server_.takeCommits(ic_), ic_.inputPanel().clientPreedit().toString()};
    }

    Server &server_;
    fcitx::ICUUID uuid_;
    fcitx::InputContext &ic_;
};

void gestures(Server &server) {
    Window w{server};
    // A mapped key shows as preedit; the leader commits its only variant;
    // the release of the committed key is swallowed.
    expect(w.press(kS), {.taken = true, .commit = "", .preedit = "s"});
    expect(w.press(kAlt), {.taken = true, .commit = "ß", .preedit = ""});
    expect(w.release(kS, fcitx::KeyState::Alt), {.taken = true, .commit = "", .preedit = ""});
    // The release of the leader is swallowed too.
    expect(w.release(kAlt, fcitx::KeyState::Alt), {.taken = true, .commit = "", .preedit = ""});

    // Unmapped keys reach the application.
    expect(w.press(kX), {});
    static_cast<void>(w.release(kX));

    // Uppercase letters have no default mapping.
    expect(w.press(kShift), {});
    expect(w.press(kShiftA, fcitx::KeyState::Shift), {});
    static_cast<void>(w.release(kShiftA, fcitx::KeyState::Shift));
    static_cast<void>(w.release(kShift));

    // A key with several variants cycles on each leader press; releasing
    // the key commits the current one.
    expect(w.press(kA), {.taken = true, .commit = "", .preedit = "a"});
    expect(w.press(kAlt), {.taken = true, .commit = "", .preedit = "ä"});
    static_cast<void>(w.release(kAlt, fcitx::KeyState::Alt));
    expect(w.press(kAlt), {.taken = true, .commit = "", .preedit = "á"});
    static_cast<void>(w.release(kAlt, fcitx::KeyState::Alt));
    expect(w.release(kA, fcitx::KeyState::Alt), {.taken = true, .commit = "á", .preedit = ""});
}

void windows(Server &server) {
    // A key held in one window is not held in another, where the leader is
    // just a modifier.
    Window first{server};
    Window second{server};
    expect(first.press(kS), {.taken = true, .commit = "", .preedit = "s"});
    expect(second.press(kAlt), {});
    expect(second.release(kAlt, fcitx::KeyState::Alt), {});
    expect(first.press(kAlt), {.taken = true, .commit = "ß", .preedit = ""});
    static_cast<void>(first.release(kS, fcitx::KeyState::Alt));
    static_cast<void>(first.release(kAlt, fcitx::KeyState::Alt));
}

void pendingKey(Server &server) {
    Window w{server};
    // Switching to another input method commits the pending key...
    expect(w.press(kO), {.taken = true, .commit = "", .preedit = "o"});
    expect(w.switchTo(kKeyboard), {.taken = false, .commit = "o", .preedit = ""});
    static_cast<void>(w.release(kO));
    static_cast<void>(w.switchTo(kAddon));

    // ...and so does leaving the window, once: Fcitx5 commits the preedit,
    // the addon only forgets the key.
    expect(w.press(kO), {.taken = true, .commit = "", .preedit = "o"});
    expect(w.focusOut(), {.taken = false, .commit = "o", .preedit = ""});
    expect(w.focusIn(), {});
    static_cast<void>(w.release(kO));

    // When the application resets its input, the key is dropped, and the
    // leader is just a modifier again.
    expect(w.press(kO), {.taken = true, .commit = "", .preedit = "o"});
    expect(w.reset(), {});
    expect(w.press(kAlt), {});
    static_cast<void>(w.release(kAlt, fcitx::KeyState::Alt));
    static_cast<void>(w.release(kO));
}

/// Make sure a rewrite of `path` is noticed: a coarse file system clock
/// could give it the time of the creation.
void touch(const std::filesystem::path &path) {
    std::filesystem::last_write_time(path,
                                     std::filesystem::last_write_time(path) + std::chrono::hours{1});
}

/// Emoji of several code points: the farmer, a ZWJ sequence (three code
/// points, eleven bytes of UTF-8), and the French flag, a pair of regional
/// indicators (eight bytes).
constexpr std::string_view kFarmer = "🧑‍🌾";
constexpr std::string_view kFrance = "🇫🇷";

/// Map `t` to the farmer and the flag in the mappings file next to
/// `config`, as the editor would.
bool mapEmoji(const std::filesystem::path &config) {
    const auto mappings = config.parent_path() / "mappings.toml";
    if (!std::filesystem::exists(mappings)) {
        return false;
    }
    std::ofstream{mappings, std::ios::app} << std::format(
        "\n[profiles.Default.mappings.t]\ncharacters = [\"{}\", \"{}\"]\n", kFarmer, kFrance);
    touch(mappings);
    return true;
}

void emoji(Server &server, const std::filesystem::path &config) {
    // Variants are bytes to the addon and to Fcitx5, whatever their length:
    // a sequence of several code points shows whole as preedit (the cursor
    // is at its end, in bytes) and commits whole.
    Window w{server};
    check(mapEmoji(config), "map t to emoji");
    static_cast<void>(w.focusOut());
    static_cast<void>(w.focusIn());
    expect(w.press(kT), {.taken = true, .commit = "", .preedit = "t"});
    expect(w.press(kAlt), {.taken = true, .commit = "", .preedit = std::string{kFarmer}});
    static_cast<void>(w.release(kAlt, fcitx::KeyState::Alt));
    expect(w.press(kAlt), {.taken = true, .commit = "", .preedit = std::string{kFrance}});
    static_cast<void>(w.release(kAlt, fcitx::KeyState::Alt));
    expect(w.release(kT, fcitx::KeyState::Alt),
           {.taken = true, .commit = std::string{kFrance}, .preedit = ""});
}

/// Blacklist `program` in the configuration file, as the editor would.
bool blacklist(const std::filesystem::path &config, std::string_view program) {
    std::string text;
    {
        std::ifstream in{config};
        text.assign(std::istreambuf_iterator<char>{in}, {});
    }
    constexpr std::string_view none = "filtered_apps = []";
    const auto at = text.find(none);
    if (at == std::string::npos) {
        return false;
    }
    text.replace(at, none.size(), std::format("filtered_apps = [\"{}\"]", program));
    std::ofstream{config} << text;
    touch(config);
    return true;
}

void reloadOnFocus(Server &server, const std::filesystem::path &config) {
    Window w{server};
    expect(w.press(kA), {.taken = true, .commit = "", .preedit = "a"});
    expect(w.release(kA), {.taken = true, .commit = "a", .preedit = ""});

    // A configuration saved meanwhile applies from the next focus on.
    check(blacklist(config, kProgram), "blacklist the test program");
    expect(w.press(kA), {.taken = true, .commit = "", .preedit = "a"});
    expect(w.release(kA), {.taken = true, .commit = "a", .preedit = ""});
    static_cast<void>(w.focusOut());
    static_cast<void>(w.focusIn());
    expect(w.press(kA), {});
    expect(w.release(kA), {});
}

/// Use the keyboard layout when inactive, and our input method when active.
void useAddon(fcitx::Instance &instance) {
    auto group = instance.inputMethodManager().currentGroup();
    group.inputMethodList() = {fcitx::InputMethodGroupItem{std::string{kKeyboard}},
                               fcitx::InputMethodGroupItem{std::string{kAddon}}};
    group.setDefaultInputMethod(std::string{kAddon});
    instance.inputMethodManager().setGroup(std::move(group));
}

void run(fcitx::Instance &instance, const std::filesystem::path &config) {
    auto &addons = instance.addonManager();
    auto *const frontend = addons.addon("testfrontend");
    check(frontend != nullptr, "testfrontend is loaded");
    check(addons.addon(std::string{kAddon}, true) != nullptr, "the addon is loaded");
    if (frontend == nullptr || failures != 0) {
        return;
    }
    useAddon(instance);

    Server server{instance, *frontend};
    gestures(server);
    windows(server);
    pendingKey(server);
    emoji(server, config);
    reloadOnFocus(server, config);
}

} // namespace

int main(int argc, char *argv[]) {
    if (argc != 4) {
        std::println(stderr, "usage: fcitx_test <addon dir> <data dir> <config file>");
        return EXIT_FAILURE;
    }
    const std::filesystem::path addonDir{argv[1]};
    const std::filesystem::path dataDir{argv[2]};
    const std::filesystem::path config{argv[3]};
    fcitx::setupTestingEnvironmentPath(addonDir, {addonDir}, {dataDir});

    std::string name{"fcitx_test"};
    std::string disable{"--disable=all"};
    std::string enable{std::format("--enable=testim,testfrontend,{}", kAddon)};
    std::array args{name.data(), disable.data(), enable.data()};
    fcitx::Instance instance{static_cast<int>(args.size()), args.data()};
    instance.addonManager().registerDefaultLoader(nullptr);

    fcitx::EventDispatcher dispatcher;
    dispatcher.attach(&instance.eventLoop());
    dispatcher.schedule([&] {
        run(instance, config);
        dispatcher.detach();
        instance.exit();
    });
    static_cast<void>(instance.exec());
    return finish("fcitx_test");
}
