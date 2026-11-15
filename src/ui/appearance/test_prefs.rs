//! A test's own appearance prefs. Tests share one settings folder per
//! process (`quill::settings::use_isolated_app_root`: the first caller wins),
//! and every app a test builds loads its appearance prefs from there. A test
//! that saves a pref that moves the layout (the density) would hand it to
//! the apps the tests running beside it build. Such a test keeps its
//! appearance prefs in a folder of its own while the guard lives: GPUI tests
//! run on their own thread, and so does every app they build.

use std::cell::Cell;
use std::path::PathBuf;

thread_local! {
    static ROOT: Cell<Option<PathBuf>> = const { Cell::new(None) };
}

/// This thread's own appearance prefs folder, if a test chose one.
pub(super) fn root() -> Option<PathBuf> {
    ROOT.with(|root| {
        let path = root.take();
        root.set(path.clone());
        path
    })
}

/// Until dropped, this thread saves and loads its appearance prefs in a fresh
/// folder named after `prefix`.
#[must_use]
pub(in crate::ui) struct OwnAppearancePrefs;

pub(in crate::ui) fn own_appearance_prefs(prefix: &str) -> OwnAppearancePrefs {
    let dir = quill::scratch::process_dir(prefix);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    ROOT.with(|root| root.set(Some(dir)));
    OwnAppearancePrefs
}

impl Drop for OwnAppearancePrefs {
    fn drop(&mut self) {
        ROOT.with(|root| root.set(None));
    }
}
