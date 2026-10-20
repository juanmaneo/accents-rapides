//! The C ABI driven by the native Fcitx5 addon (`cpp/addon.cpp`), declared
//! in `cpp/accents_rapides.hpp`. Keep both files in sync.

use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;
use tracing::error;

use crate::{key_event, str_arg, FcitxAddon, FcitxError};
use accents_rapides_core::{Action, Session};

/// An [`Action`] for C. NULL strings mean none; the others are owned by the
/// addon and stay valid until its next call.
#[repr(C)]
#[derive(Debug)]
pub struct FcitxAccentsRapidesAction {
    pub commit: *const c_char,
    pub preedit: *const c_char,
    pub forward: bool,
}

/// Let the key through, for calls that cannot reach the addon.
const PASS_THROUGH: FcitxAccentsRapidesAction = FcitxAccentsRapidesAction {
    commit: ptr::null(),
    preedit: ptr::null(),
    forward: true,
};

impl FcitxAddon {
    /// Keep the strings of `action` alive in `self` and point to them.
    fn expose(&mut self, action: Action) -> FcitxAccentsRapidesAction {
        self.commit = c_string(action.commit);
        self.preedit = c_string(action.preedit);
        FcitxAccentsRapidesAction {
            commit: self.commit.as_deref().map_or(ptr::null(), |s| s.as_ptr()),
            preedit: self.preedit.as_deref().map_or(ptr::null(), |s| s.as_ptr()),
            forward: action.forward,
        }
    }
}

/// `text` as a C string; dropped (and logged) if it contains a NUL byte.
fn c_string(text: Option<String>) -> Option<CString> {
    CString::new(text?)
        .inspect_err(|e| error!("cannot pass text to Fcitx5: {e}"))
        .ok()
}

/// Drop a value boxed for C (NULL is a no-op).
///
/// # Safety
///
/// `ptr` must be NULL or come from `Box::into_raw` and not be freed yet.
unsafe fn free<T>(ptr: *mut T) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}

/// Load the configuration and create the addon state; NULL on failure.
#[no_mangle]
pub extern "C" fn fcitx_accents_rapides_create() -> *mut FcitxAddon {
    match FcitxAddon::new() {
        Ok(addon) => Box::into_raw(Box::new(addon)),
        Err(e) => {
            error!("cannot create the accents-rapides addon: {e}");
            ptr::null_mut()
        }
    }
}

/// Destroy an addon created by `fcitx_accents_rapides_create` (NULL is a
/// no-op).
///
/// # Safety
///
/// `addon` must be NULL or a pointer returned by
/// `fcitx_accents_rapides_create` that was not destroyed yet.
#[no_mangle]
pub unsafe extern "C" fn fcitx_accents_rapides_destroy(addon: *mut FcitxAddon) {
    free(addon);
}

/// Create the gesture state of one window.
#[no_mangle]
pub extern "C" fn fcitx_accents_rapides_session_create() -> *mut Session {
    Box::into_raw(Box::default())
}

/// Destroy a session created by `fcitx_accents_rapides_session_create`
/// (NULL is a no-op).
///
/// # Safety
///
/// `session` must be NULL or a pointer returned by
/// `fcitx_accents_rapides_session_create` that was not destroyed yet.
#[no_mangle]
pub unsafe extern "C" fn fcitx_accents_rapides_session_destroy(session: *mut Session) {
    free(session);
}

/// Handle a key event sent to `program`, in the window of `session`. `key`
/// is what the key types or its name (see `accents_rapides_core::key_name`),
/// `code` the physical key code and `flags` a combination of `key_flags`.
/// NULL or invalid arguments let the key through.
///
/// # Safety
///
/// `addon` and `session` must be NULL or live; `program` and `key` must be
/// NULL or valid NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn fcitx_accents_rapides_key_event(
    addon: *mut FcitxAddon,
    session: *mut Session,
    program: *const c_char,
    key: *const c_char,
    code: u32,
    flags: u32,
) -> FcitxAccentsRapidesAction {
    let (Some(addon), Some(session)) = (addon.as_mut(), session.as_mut()) else {
        return PASS_THROUGH;
    };
    let action = match (str_arg(program), str_arg(key)) {
        (Ok(program), Ok(key)) => addon.key_event(session, program, &key_event(key, code, flags)),
        (Err(e), _) | (_, Err(e)) => {
            error!("invalid key event: {e}");
            Action::pass_through()
        }
    };
    addon.expose(action)
}

/// End the gesture of `session`, returning what it would commit (for the
/// caller to commit or not).
///
/// # Safety
///
/// `addon` and `session` must be NULL or live.
#[no_mangle]
pub unsafe extern "C" fn fcitx_accents_rapides_reset(
    addon: *mut FcitxAddon,
    session: *mut Session,
) -> FcitxAccentsRapidesAction {
    match (addon.as_mut(), session.as_mut()) {
        (Some(addon), Some(session)) => {
            let action = addon.reset(session);
            addon.expose(action)
        }
        _ => PASS_THROUGH,
    }
}

/// Reload the configuration if its file changed since it was loaded; false
/// (keeping the current one) on failure.
///
/// # Safety
///
/// `addon` must be NULL or a live addon.
#[no_mangle]
pub unsafe extern "C" fn fcitx_accents_rapides_refresh(addon: *mut FcitxAddon) -> bool {
    addon
        .as_mut()
        .is_some_and(|addon| reloaded(addon.refresh()))
}

/// Reload the configuration and mappings; false (keeping the current ones)
/// on failure.
///
/// # Safety
///
/// `addon` must be NULL or a live addon.
#[no_mangle]
pub unsafe extern "C" fn fcitx_accents_rapides_reload(addon: *mut FcitxAddon) -> bool {
    addon.as_mut().is_some_and(|addon| reloaded(addon.reload()))
}

/// Whether a reload succeeded (or was not needed), logging its failure.
fn reloaded<T>(result: Result<T, FcitxError>) -> bool {
    result
        .inspect_err(|e| error!("cannot reload accents-rapides: {e}"))
        .is_ok()
}

#[cfg(test)]
mod tests {
    // The C ABI is exercised from the lib's own unit tests so that every
    // compiled copy of these #[no_mangle] symbols is referenced and its
    // coverage counters recorded (unreferenced symbols are dropped from
    // test binaries, which otherwise leaves stale zero-count records).
    use super::*;
    use crate::key_flags::{MODIFIER, RELEASE};
    use accents_rapides_core::{KeyMapping, MappingKey, MappingProfile};
    use accents_rapides_test_support::{with_temp_config, TempConfig};
    use std::ffi::{CStr, CString};

    /// The strings of an action, copied.
    fn strings(action: &FcitxAccentsRapidesAction) -> (Option<String>, Option<String>) {
        let copy = |ptr: *const c_char| {
            (!ptr.is_null()).then(|| {
                unsafe { CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned()
            })
        };
        (copy(action.commit), copy(action.preedit))
    }

    /// Send `key` with `flags` to `addon` in the window of `session`, from a
    /// program named "test".
    unsafe fn send(
        addon: *mut FcitxAddon,
        session: *mut Session,
        key: &str,
        flags: u32,
    ) -> FcitxAccentsRapidesAction {
        let program = CString::new("test").unwrap();
        let key = CString::new(key).unwrap();
        fcitx_accents_rapides_key_event(addon, session, program.as_ptr(), key.as_ptr(), 30, flags)
    }

    /// Press the default leader, Alt (a modifier, on a key of its own).
    unsafe fn leader(addon: *mut FcitxAddon, session: *mut Session) -> FcitxAccentsRapidesAction {
        fcitx_accents_rapides_key_event(
            addon,
            session,
            c"test".as_ptr(),
            c"alt".as_ptr(),
            64,
            MODIFIER,
        )
    }

    /// Run `f` with a fresh addon and session, then destroy both.
    fn with_addon(f: impl FnOnce(*mut FcitxAddon, *mut Session)) {
        with_temp_config(|| unsafe {
            let addon = fcitx_accents_rapides_create();
            assert!(!addon.is_null());
            let session = fcitx_accents_rapides_session_create();
            f(addon, session);
            fcitx_accents_rapides_session_destroy(session);
            fcitx_accents_rapides_destroy(addon);
        });
    }

    #[test]
    fn c_abi_gesture_cycle() {
        with_addon(|addon, session| unsafe {
            // Pressing "a" shows it as preedit and swallows the key.
            let action = send(addon, session, "a", 0);
            assert_eq!(strings(&action), (None, Some("a".to_string())));
            assert!(!action.forward);

            // The leader commits the default mapping's only variant.
            let action = leader(addon, session);
            assert_eq!(strings(&action), (Some("ä".to_string()), None));
            assert!(!action.forward);

            // The release of the committed key is swallowed too.
            let action = send(addon, session, "a", RELEASE);
            assert_eq!(strings(&action), (None, None));
            assert!(!action.forward);

            // Any other key goes through.
            assert!(send(addon, session, "x", 0).forward);
        });
        // Destroying NULL is a no-op.
        unsafe {
            fcitx_accents_rapides_destroy(ptr::null_mut());
            fcitx_accents_rapides_session_destroy(ptr::null_mut());
        }
    }

    #[test]
    fn c_abi_sessions_are_independent() {
        with_addon(|addon, session| unsafe {
            let other = fcitx_accents_rapides_session_create();

            // "a" is held in one window: the leader in another one is just
            // a modifier, and resetting that one hands back nothing.
            send(addon, session, "a", 0);
            assert!(leader(addon, other).forward);
            assert_eq!(
                strings(&fcitx_accents_rapides_reset(addon, other)),
                (None, None)
            );
            assert_eq!(strings(&leader(addon, session)).0.as_deref(), Some("ä"));

            fcitx_accents_rapides_session_destroy(other);
        });
    }

    #[test]
    fn c_abi_reset_refresh_and_reload() {
        with_addon(|addon, session| unsafe {
            // Reset hands back the pending key for the caller to commit.
            send(addon, session, "a", 0);
            let action = fcitx_accents_rapides_reset(addon, session);
            assert_eq!(strings(&action), (Some("a".to_string()), None));
            assert_eq!(
                strings(&fcitx_accents_rapides_reset(addon, session)),
                (None, None)
            );

            assert!(fcitx_accents_rapides_refresh(addon));
            assert!(fcitx_accents_rapides_reload(addon));
        });
    }

    #[test]
    fn c_abi_reload_failures_keep_the_addon() {
        let temp = TempConfig::new();
        let addon = temp.with(|| fcitx_accents_rapides_create());
        let session = fcitx_accents_rapides_session_create();

        // A configuration that cannot be read anymore.
        std::fs::write(&temp.path, "not = = toml").unwrap();
        assert!(!temp.with(|| unsafe { fcitx_accents_rapides_refresh(addon) }));
        let failed =
            TempConfig::unwritable().with(|| unsafe { fcitx_accents_rapides_reload(addon) });
        assert!(!failed);

        // The addon still works with its previous configuration.
        temp.with(|| unsafe {
            assert_eq!(
                strings(&send(addon, session, "a", 0)).1.as_deref(),
                Some("a")
            );
            fcitx_accents_rapides_session_destroy(session);
            fcitx_accents_rapides_destroy(addon);
        });
    }

    #[test]
    fn c_abi_null_and_invalid_arguments() {
        with_addon(|addon, session| unsafe {
            let null = ptr::null_mut();
            let key = CString::new("a").unwrap();
            let pass = |action: FcitxAccentsRapidesAction| {
                action.forward && action.commit.is_null() && action.preedit.is_null()
            };

            // Without an addon or a session everything passes through or
            // fails cleanly.
            for (addon, session) in [(null, session), (addon, null.cast())] {
                assert!(pass(fcitx_accents_rapides_key_event(
                    addon,
                    session,
                    key.as_ptr(),
                    key.as_ptr(),
                    1,
                    0
                )));
                assert!(pass(fcitx_accents_rapides_reset(addon, session)));
            }
            assert!(!fcitx_accents_rapides_refresh(null));
            assert!(!fcitx_accents_rapides_reload(null));

            // NULL or invalid UTF-8 strings let the key through.
            let bad = c"\xff\xfe".as_ptr();
            for (program, key) in [
                (ptr::null(), key.as_ptr()),
                (key.as_ptr(), ptr::null()),
                (key.as_ptr(), bad),
            ] {
                assert!(pass(fcitx_accents_rapides_key_event(
                    addon, session, program, key, 1, 0
                )));
            }
        });
    }

    #[test]
    fn c_abi_create_failure_returns_null() {
        // A config location that cannot be created makes create() fail: the
        // C side must get NULL, not a crash.
        let addon = TempConfig::unwritable().with(|| fcitx_accents_rapides_create());
        assert!(addon.is_null());
    }

    #[test]
    fn c_abi_drops_text_with_nul_bytes() {
        with_addon(|addon, session| unsafe {
            // A variant with an interior NUL cannot become a C string: it is
            // dropped rather than truncated.
            let mut profile = MappingProfile::new("Nul");
            profile.mappings.insert(
                MappingKey::Char('z'),
                KeyMapping::new(vec!["a\0b".to_string()]),
            );
            let manager = (*addon).handler_mut().mapping_manager_mut();
            manager.add_profile(profile);
            assert!(manager.set_current_profile("Nul"));

            send(addon, session, "z", 0);
            let action = leader(addon, session);
            assert!(action.commit.is_null());
            assert!(!action.forward);
        });
    }
}
