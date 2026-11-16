//! The "Jump to date" box clears its open flag when its dialog closes
//! (needs the `demo-capture` feature for gpui-kit's test support).
use crate::ui::app::QuillApp;
use crate::ui::keybindings::bind_keys;
use crate::ui::screenshot_demo::ScreenshotDemo;
use crate::ui::shell::{DialogKind, QuillShell};
use gpui_kit::component::Root;
use gpui_kit::{AppContext, Entity, TestAppContext, VisualTestContext, px, size};

fn new_app(cx: &mut TestAppContext) -> (Entity<QuillApp>, Entity<QuillShell>, VisualTestContext) {
    cx.update(gpui_kit::init);
    cx.update(bind_keys);
    let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot_in = slot.clone();
    let handle = cx.open_window(size(px(1100.), px(800.)), move |window, cx| {
        let view = cx.new(|cx| {
            QuillApp::new_with_demo(window, cx, None, Some(ScreenshotDemo::named("ready-chats")))
        });
        let shell = cx.new(|_| QuillShell::new(view.clone()));
        *slot_in.borrow_mut() = Some((view, shell.clone()));
        Root::new(shell, window, cx)
    });
    let (app, shell) = slot.borrow().clone().expect("the app view");
    let vcx = VisualTestContext::from_window(handle.into(), cx);
    (app, shell, vcx)
}

fn calendar_open(app: &Entity<QuillApp>, vcx: &mut VisualTestContext) -> bool {
    app.read_with(vcx, |app, _| {
        app.session()
            .is_some_and(|s| s.search.history_calendar.is_some())
    })
}

#[gpui_kit::test]
fn the_dialog_close_clears_the_open_calendar(cx: &mut TestAppContext) {
    let (app, shell, mut vcx) = new_app(cx);
    let vcx = &mut vcx;
    vcx.run_until_parked();
    app.update_in(vcx, |app, _, cx| app.open_jump_date_ui(cx));
    vcx.run_until_parked();
    assert!(calendar_open(&app, vcx));
    shell.read_with(vcx, |shell, _| {
        assert_eq!(shell.open_dialog, Some(DialogKind::JumpToDate));
    });
    app.update_in(vcx, |app, _, cx| app.close_jump_date_ui(cx));
    vcx.run_until_parked();
    assert!(!calendar_open(&app, vcx));
    shell.read_with(vcx, |shell, _| assert_eq!(shell.open_dialog, None));
}

#[test]
fn the_russian_calendar_has_seven_distinct_weekday_headers() {
    use quill::lang::{Snapshot, install, isolate_thread};
    use std::collections::HashSet;

    isolate_thread();
    let entries = quill::lang::parse::parse_strings(include_str!("../../lang/ru/misc.strings"))
        .expect("the Russian strings file parses");
    let mut pack = Snapshot::new("ru", "ru", false);
    pack.layer_entries(&entries);
    install(Some(pack));
    let headers = super::weekday_headers();
    install(None);

    let unique: HashSet<&str> = headers.iter().copied().collect();
    assert_eq!(
        headers.len(),
        7,
        "the header has seven columns: {headers:?}"
    );
    assert_eq!(unique.len(), 7, "each weekday differs: {headers:?}");
    assert!(
        headers.iter().all(|label| label.chars().count() >= 2),
        "Cyrillic short names are not byte-sliced: {headers:?}"
    );
}
