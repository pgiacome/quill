//! Widgets stay off, and no widget code runs, until the warning is accepted
//! with its box ticked; turning them off stops every runtime.

use crate::ui::app::QuillApp;
use crate::ui::keybindings::bind_keys;
use crate::ui::screenshot_demo::ScreenshotDemo;
use crate::ui::shell::{DialogKind, QuillShell};
use crate::ui::widgets_state::Consent;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext, px, size};
use quill::message_widget::Permission;

fn new_app(
    cx: &mut TestAppContext,
    demo: &'static str,
) -> (Entity<QuillApp>, Entity<QuillShell>, VisualTestContext) {
    cx.update(gpui_kit::init);
    cx.update(bind_keys);
    let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot_in = slot.clone();
    let demo = ScreenshotDemo::named(demo);
    let handle = cx.open_window(size(px(1100.), px(900.)), move |window, cx| {
        let view = cx.new(|cx| QuillApp::new_with_demo(window, cx, None, Some(demo)));
        let shell = cx.new(|_| QuillShell::new(view.clone()));
        *slot_in.borrow_mut() = Some((view, shell.clone()));
        Root::new(shell, window, cx)
    });
    let (app, shell) = slot.borrow().clone().expect("the app view");
    let vcx = VisualTestContext::from_window(handle.into(), cx);
    (app, shell, vcx)
}

fn frames(vcx: &mut VisualTestContext) {
    vcx.update(|_, cx| cx.set_reduce_motion(true));
    vcx.run_until_parked();
    for _ in 0..2 {
        vcx.update(|window, cx| window.render_frame(cx));
    }
    vcx.run_until_parked();
}

fn find(vcx: &mut VisualTestContext, selector: &'static str) -> bool {
    vcx.debug_bounds(selector).is_some()
}

fn click(vcx: &mut VisualTestContext, selector: &'static str) {
    let bounds = vcx.debug_bounds(selector).expect(selector);
    vcx.simulate_click(bounds.center(), Modifiers::default());
    frames(vcx);
}

fn dialog(shell: &Entity<QuillShell>, vcx: &mut VisualTestContext) -> Option<DialogKind> {
    shell.read_with(&*vcx, |shell, _| shell.open_dialog.kind())
}

/// Widgets that ever started: an entry exists only once a pass noted one.
fn noted(app: &Entity<QuillApp>, vcx: &mut VisualTestContext) -> usize {
    app.read_with(&*vcx, |app, _| app.widgets.entries.len())
}

fn live(app: &Entity<QuillApp>, vcx: &mut VisualTestContext) -> usize {
    app.read_with(&*vcx, |app, _| {
        app.widgets
            .entries
            .values()
            .filter(|entry| entry.lifecycle.is_live())
            .count()
    })
}

#[gpui_kit::test]
fn widgets_run_only_after_the_warning_is_accepted(cx: &mut TestAppContext) {
    let _prefs = crate::ui::appearance::own_appearance_prefs("widget-consent-accept");
    let (app, shell, mut vcx) = new_app(cx, "ready-widgets-off");
    frames(&mut vcx);
    assert!(
        find(&mut vcx, "widgets-off-card"),
        "the bubble shows the notice"
    );
    assert_eq!(
        noted(&app, &mut vcx),
        0,
        "no widget is noted, so none starts"
    );

    click(&mut vcx, "widgets-off-turn-on");
    assert_eq!(dialog(&shell, &mut vcx), Some(DialogKind::WidgetConsent));

    // The confirm does nothing while "I understand" is clear.
    app.update(&mut vcx, |app, cx| app.allow_widgets(cx));
    frames(&mut vcx);
    assert!(app.read_with(&vcx, |app, _| !app.widgets_allowed()));
    assert_eq!(dialog(&shell, &mut vcx), Some(DialogKind::WidgetConsent));

    click(&mut vcx, "widget-consent-understand");
    assert_eq!(
        app.read_with(&vcx, |app, _| app.widgets.consent),
        Consent::Understood
    );
    click(&mut vcx, "widget-consent-confirm");
    assert!(app.read_with(&vcx, |app, _| app.widgets_allowed()));
    assert_eq!(dialog(&shell, &mut vcx), None);
    assert!(!find(&mut vcx, "widgets-off-card"));
    assert!(noted(&app, &mut vcx) > 0, "the shown widgets start");
    assert_eq!(
        app.read_with(&vcx, |app, _| app.chat_prefs.message_widgets),
        Permission::On,
        "the choice is saved with the app's prefs"
    );
    let saved = quill::settings::load_chat_prefs(&QuillApp::appearance_paths());
    assert_eq!(saved.message_widgets, Permission::On);
}

#[gpui_kit::test]
fn cancel_leaves_widgets_off(cx: &mut TestAppContext) {
    let _prefs = crate::ui::appearance::own_appearance_prefs("widget-consent-cancel");
    let (app, shell, mut vcx) = new_app(cx, "ready-widgets-off");
    frames(&mut vcx);
    click(&mut vcx, "widgets-off-turn-on");
    click(&mut vcx, "widget-consent-understand");
    click(&mut vcx, "widget-consent-cancel");
    assert_eq!(dialog(&shell, &mut vcx), None);
    assert!(app.read_with(&vcx, |app, _| !app.widgets_allowed()));
    assert!(find(&mut vcx, "widgets-off-card"));
    assert_eq!(noted(&app, &mut vcx), 0);
    // Opened again, the box starts clear.
    click(&mut vcx, "widgets-off-turn-on");
    assert_eq!(
        app.read_with(&vcx, |app, _| app.widgets.consent),
        Consent::Asking
    );
}

#[gpui_kit::test]
fn turning_widgets_off_stops_every_runtime(cx: &mut TestAppContext) {
    let _prefs = crate::ui::appearance::own_appearance_prefs("widget-consent-off");
    let (app, _shell, mut vcx) = new_app(cx, "ready-widgets");
    frames(&mut vcx);
    assert!(live(&app, &mut vcx) > 0, "the demo's widgets run");
    app.update(&mut vcx, |app, cx| app.turn_off_widgets(cx));
    frames(&mut vcx);
    assert_eq!(live(&app, &mut vcx), 0);
    assert!(app.read_with(&vcx, |app, _| {
        app.widgets
            .entries
            .values()
            .all(|entry| entry.frame.is_none())
    }));
    assert!(find(&mut vcx, "widgets-off-card"));
    let saved = quill::settings::load_chat_prefs(&QuillApp::appearance_paths());
    assert_eq!(saved.message_widgets, Permission::Off);
}
