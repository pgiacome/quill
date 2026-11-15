//! The forum's topic rows through GPUI: a fixed chat-row height, the round
//! topic icon before the text, and the unread badge on the second line
//! under the timestamp — never over the preview.
#![cfg(test)]
#![cfg(feature = "demo-capture")]

use crate::ui::app::QuillApp;
use crate::ui::screenshot_demo::ScreenshotDemo;
use crate::ui::shell::QuillShell;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Bounds, Entity, Pixels, TestAppContext, VisualTestContext, px, size};

fn frames(vcx: &mut VisualTestContext) {
    vcx.run_until_parked();
    for _ in 0..3 {
        vcx.update(|window, cx| window.render_frame(cx));
    }
}

/// The `ready-topic-loading` demo opens the Dev Forum with its topics
/// seeded, at a width that keeps the topic column beside the conversation.
fn open(cx: &mut TestAppContext) -> (Entity<QuillApp>, VisualTestContext) {
    cx.update(gpui_kit::init);
    let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot_in = slot.clone();
    let demo = ScreenshotDemo::named("ready-topic-loading");
    let handle = cx.open_window(size(px(1280.), px(800.)), move |window, cx| {
        let view = cx.new(|cx| QuillApp::new_with_demo(window, cx, None, Some(demo)));
        *slot_in.borrow_mut() = Some(view.clone());
        let shell = cx.new(|_| QuillShell::new(view));
        Root::new(shell, window, cx)
    });
    let app: Entity<QuillApp> = slot.borrow().clone().expect("the app");
    let mut vcx = VisualTestContext::from_window(handle.into(), cx);
    frames(&mut vcx);
    // `ready-topic-loading` seeds `has_forum_tabs`, which draws the topics
    // as tabs instead of the column this test is about. Drop the tabs mode
    // so the forum column, with the topic rows, is drawn.
    app.update(&mut vcx, |app, cx| {
        if let Some(session) = app.demo_session.as_mut() {
            session.threads.forum_tabs_supergroups.remove(&16);
        }
        cx.notify();
    });
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

#[gpui_kit::test]
fn topic_rows_put_the_time_and_the_badge_at_the_trailing_edge(cx: &mut TestAppContext) {
    let (_app, mut vcx) = open(cx);
    let vcx = &mut vcx;
    let row = shown(vcx, ("forum-topic-row", 3));
    let next = shown(vcx, ("forum-topic-row", 2));
    let name = shown(vcx, ("forum-topic-name", 3));
    let stamp = shown(vcx, ("forum-topic-stamp", 3));
    let preview = shown(vcx, ("forum-topic-preview", 3));
    let badge = shown(vcx, ("forum-topic-unread", 3));

    // One chat-row tall, stacked down the topic column.
    assert_eq!(row.size.height, px(quill::density::STANDARD.row_base));
    assert!(next.origin.y >= row.bottom());
    // The round icon leads; the name starts after it.
    assert!(name.origin.x >= row.origin.x + px(42.));
    // The name truncates before the timestamp, which holds the right edge.
    assert!(name.right() <= stamp.origin.x + px(1.));
    assert!(stamp.right() > row.right() - px(20.));
    // The preview is the second line; the badge trails it on that line.
    assert!(preview.origin.y >= name.bottom());
    assert!(badge.origin.y >= name.bottom());
    assert!(badge.left() >= preview.right());
    // The badge sits under the timestamp, both at the trailing edge.
    assert!((badge.right() - stamp.right()).abs() <= px(1.));
    assert!(badge.right() <= row.right());
}

#[gpui_kit::test]
fn a_read_topic_has_no_unread_badge(cx: &mut TestAppContext) {
    let (_app, mut vcx) = open(cx);
    let _ = shown(&mut vcx, ("forum-topic-row", 4));
    assert!(
        vcx.update(|window, _| window.try_find(("forum-topic-unread", 4u64)).is_none()),
        "a read topic drew an unread badge"
    );
}
