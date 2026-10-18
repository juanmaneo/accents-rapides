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
constexpr PhysicalKey kO{FcitxKey_o, 32};
constexpr PhysicalKey kX{FcitxKey_x, 53};
constexpr PhysicalKey kSpace{FcitxKey_space, 65};
constexpr PhysicalKey kShift{FcitxKey_Shift_L, 50};

static_assert(kA.code == kShiftA.code, "Shift changes the symbol, not the key");
static_assert(kA.code != kO.code && kO.code != kX.code && kX.code != kSpace.code);

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
    expect(w.press(kA), {.taken = true, .commit = "", .preedit = "a"});
    expect(w.press(kSpace), {.taken = true, .commit = "ä", .preedit = ""});
    expect(w.release(kA), {.taken = true, .commit = "", .preedit = ""});
    static_cast<void>(w.release(kSpace));

    // Unmapped keys reach the application.
    expect(w.press(kX), {});
    static_cast<void>(w.release(kX));

    // Shift selects the uppercase mapping.
    expect(w.press(kShift), {});
    expect(w.press(kShiftA, fcitx::KeyState::Shift), {.taken = true, .commit = "", .preedit = "A"});
    expect(w.press(kSpace, fcitx::KeyState::Shift), {.taken = true, .commit = "Ä", .preedit = ""});
    static_cast<void>(w.release(kShiftA, fcitx::KeyState::Shift));
    static_cast<void>(w.release(kSpace, fcitx::KeyState::Shift));
    static_cast<void>(w.release(kShift, fcitx::KeyState::Shift));
}

void windows(Server &server) {
    // A key held in one window is not held in another, where the leader is
    // an ordinary key.
    Window first{server};
    Window second{server};
    expect(first.press(kO), {.taken = true, .commit = "", .preedit = "o"});
    expect(second.press(kSpace), {});
    static_cast<void>(second.release(kSpace));
    expect(first.press(kSpace), {.taken = true, .commit = "ö", .preedit = ""});
    static_cast<void>(first.release(kO));
    static_cast<void>(first.release(kSpace));
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
    // leader is an ordinary key again.
    expect(w.press(kO), {.taken = true, .commit = "", .preedit = "o"});
    expect(w.reset(), {});
    expect(w.press(kSpace), {});
    static_cast<void>(w.release(kSpace));
    static_cast<void>(w.release(kO));
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
    // A coarse file system clock could give the rewrite the time of the
    // creation, and the change would go unnoticed.
    std::filesystem::last_write_time(
        config, std::filesystem::last_write_time(config) + std::chrono::hours{1});
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
