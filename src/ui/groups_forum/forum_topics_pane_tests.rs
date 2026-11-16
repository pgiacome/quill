//! The forum's topic rows through GPUI, on the `ready-forum-topics` demo:
//! chat-row height at each density, the topic icon at the chat photo's size,
//! the time and the counters at the trailing edge, and the counters a topic's
//! state calls for.
#![cfg(test)]
#![cfg(feature = "demo-capture")]

use crate::ui::app::QuillApp;
use crate::ui::screenshot_demo::ScreenshotDemo;
use crate::ui::shell::QuillShell;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Bounds, Entity, Pixels, TestAppContext, VisualTestContext, px, size};
use quill::density::{COMPACT, Density, STANDARD};

fn frames(vcx: &mut VisualTestContext) {
    vcx.run_until_parked();
    for _ in 0..3 {
        vcx.update(|window, cx| window.render_frame(cx));
    }
}

/// The Dev Forum's topic list, at a width that keeps the topic column
/// beside the conversation.
fn open(cx: &mut TestAppContext) -> (Entity<QuillApp>, VisualTestContext) {
    cx.update(gpui_kit::init);
    let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot_in = slot.clone();
    let demo = ScreenshotDemo::named("ready-forum-topics");
    let handle = cx.open_window(size(px(1280.), px(800.)), move |window, cx| {
        let view = cx.new(|cx| QuillApp::new_with_demo(window, cx, None, Some(demo)));
        *slot_in.borrow_mut() = Some(view.clone());
        let shell = cx.new(|_| QuillShell::new(view));
        Root::new(shell, window, cx)
    });
    let app: Entity<QuillApp> = slot.borrow().clone().expect("the app");
    let mut vcx = VisualTestContext::from_window(handle.into(), cx);
    frames(&mut vcx);
    (app, vcx)
}

fn shown(vcx: &mut VisualTestContext, id: (&'static str, u64)) -> Bounds<Pixels> {
    vcx.update(|window, _| {
        let target = window.find(id);
        assert!(target.visible(), "{id:?} is not visible");
        target.bounds()
    })
}

fn absent(vcx: &mut VisualTestContext, id: (&'static str, u64)) -> bool {
    vcx.update(|window, _| window.try_find(id).is_none())
}

/// Topic 3 has three unread messages and the longest name.
fn assert_row_layout(vcx: &mut VisualTestContext, metrics: &quill::density::DensityMetrics) {
    let row = shown(vcx, ("forum-topic-row", 3));
    let next = shown(vcx, ("forum-topic-row", 2));
    let name = shown(vcx, ("forum-topic-name", 3));
    let stamp = shown(vcx, ("forum-topic-stamp", 3));
    let preview = shown(vcx, ("forum-topic-preview", 3));
    let badge = shown(vcx, ("forum-topic-unread", 3));

    // One chat row tall, stacked down the topic column.
    assert_eq!(row.size.height, px(metrics.row_base));
    assert!(next.origin.y >= row.bottom());
    // The icon is the chat photo's size, and the name starts after it.
    assert!(name.origin.x >= row.origin.x + px(metrics.row_inset + metrics.avatar));
    // The name truncates before the timestamp, which holds the right edge.
    assert!(name.right() <= stamp.origin.x + px(1.));
    assert!(stamp.right() > row.right() - px(metrics.row_inset + 12.));
    // The preview is the second line; the badge trails it on that line,
    // under the timestamp, inside the row.
    assert!(preview.origin.y >= name.bottom());
    assert!(badge.origin.y >= name.bottom());
    assert!(badge.left() >= preview.right());
    assert!((badge.right() - stamp.right()).abs() <= px(1.));
    assert!(badge.top() >= row.top() && badge.bottom() <= row.bottom());
}

#[gpui_kit::test]
fn topic_rows_take_the_chat_row_shape_at_each_density(cx: &mut TestAppContext) {
    let _prefs = crate::ui::appearance::own_appearance_prefs("quill-density-forum-topics");
    let (app, mut vcx) = open(cx);
    assert_row_layout(&mut vcx, &STANDARD);

    app.update(&mut vcx, |app, cx| {
        app.set_appearance(cx, |appearance| appearance.density = Density::Compact);
    });
    frames(&mut vcx);
    assert_row_layout(&mut vcx, &COMPACT);
}

#[gpui_kit::test]
fn a_read_topic_has_no_unread_badge(cx: &mut TestAppContext) {
    let (_app, mut vcx) = open(cx);
    let _ = shown(&mut vcx, ("forum-topic-row", 4));
    assert!(
        absent(&mut vcx, ("forum-topic-unread", 4)),
        "a read topic drew an unread badge"
    );
}

#[gpui_kit::test]
fn a_single_unread_mention_shows_the_mention_mark_alone(cx: &mut TestAppContext) {
    let (_app, mut vcx) = open(cx);
    let _ = shown(&mut vcx, ("forum-topic-row", 5));
    assert!(
        absent(&mut vcx, ("forum-topic-unread", 5)),
        "the @ mark carries a lone unread mention, as on chat rows"
    );
}
