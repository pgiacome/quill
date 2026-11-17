//! Menu aim in the real chat-row menu: the pointer goes from "Mute
//! notifications" toward its submenu across the row below, driven by
//! simulated pointer moves and the executor's manual clock.

use super::context_menu_focus_tests::{new_app, open_chat_menu};
use crate::ui::app::QuillApp;
use crate::ui::menu_states::ChatMenuSubmenu;
use gpui_kit::component::menu::SUBMENU_AIM_REST;
use gpui_kit::{
    Bounds, Entity, Modifiers, Pixels, Point, Role, TestAppContext, VisualTestContext, point, px,
};
use quill::lang::{keys, tr};

/// The drawn rows of the menu's own column (the submenu's sit to its
/// right), top to bottom.
fn menu_rows(vcx: &mut VisualTestContext) -> Vec<(String, Bounds<Pixels>)> {
    let rows: Vec<(String, Bounds<Pixels>)> = vcx.update(|window, _| {
        gpui_base::test_support::snapshots(window)
            .into_iter()
            .filter(|row| row.role() == Some(Role::MenuItem) && row.visible())
            .map(|row| (row.label().unwrap_or_default().to_string(), row.bounds()))
            .collect()
    });
    let left = rows
        .iter()
        .map(|(_, bounds)| f32::from(bounds.left()))
        .fold(f32::MAX, f32::min);
    let mut column: Vec<_> = rows
        .into_iter()
        .filter(|(_, bounds)| f32::from(bounds.left()) <= left + 1.)
        .collect();
    column.sort_by(|a, b| f32::from(a.1.top()).total_cmp(&f32::from(b.1.top())));
    column
}

fn submenu(app: &Entity<QuillApp>, vcx: &mut VisualTestContext) -> Option<ChatMenuSubmenu> {
    app.read_with(vcx, |app, _| {
        app.message_ui.menu.row().and_then(|menu| menu.submenu)
    })
}

fn holds(app: &Entity<QuillApp>, vcx: &mut VisualTestContext) -> bool {
    app.read_with(vcx, |app, _| app.frame.menu.items.aim.holds())
}

fn move_to(vcx: &mut VisualTestContext, position: Point<Pixels>) {
    vcx.simulate_mouse_move(position, None, Modifiers::default());
    vcx.run_until_parked();
}

/// Opens the chat menu and the Mute submenu by pointing at its row's middle.
/// Returns the Mute row and the row under it.
fn point_at_mute(
    app: &Entity<QuillApp>,
    vcx: &mut VisualTestContext,
) -> (Bounds<Pixels>, Bounds<Pixels>) {
    open_chat_menu(app, vcx);
    let label = tr(keys::Q_CHAT_MUTE_NOTIFICATIONS).to_string();
    let rows = menu_rows(vcx);
    let at = rows
        .iter()
        .position(|(name, _)| *name == label)
        .unwrap_or_else(|| panic!("no Mute row in {rows:?}"));
    let mute = rows[at].1;
    let below = rows
        .get(at + 1)
        .map(|(_, bounds)| *bounds)
        .expect("a row under Mute");
    move_to(vcx, mute.center());
    // A second move on the row: the first one draws the submenu, which the
    // aim then measures.
    move_to(vcx, mute.center() + point(px(1.), px(0.)));
    assert_eq!(submenu(app, vcx), Some(ChatMenuSubmenu::Mute));
    (mute, below)
}

#[gpui_kit::test]
fn heading_for_the_submenu_across_a_row_keeps_it_open(cx: &mut TestAppContext) {
    let (app, mut vcx) = new_app(cx);
    let vcx = &mut vcx;
    let (_, below) = point_at_mute(&app, vcx);
    // Down and right, onto the next row near the menu's edge.
    move_to(vcx, point(below.right() - px(6.), below.top() + px(4.)));
    assert_eq!(submenu(&app, vcx), Some(ChatMenuSubmenu::Mute));
    assert!(holds(&app, vcx), "the row under waits");
}

#[gpui_kit::test]
fn resting_on_the_way_hands_the_hover_to_the_row_under(cx: &mut TestAppContext) {
    let (app, mut vcx) = new_app(cx);
    let vcx = &mut vcx;
    let (_, below) = point_at_mute(&app, vcx);
    move_to(vcx, point(below.right() - px(6.), below.top() + px(4.)));
    vcx.executor().advance_clock(SUBMENU_AIM_REST);
    vcx.run_until_parked();
    assert_eq!(submenu(&app, vcx), None, "the row under took the hover");
    assert!(!holds(&app, vcx));
}

#[gpui_kit::test]
fn moving_straight_down_closes_the_submenu_at_once(cx: &mut TestAppContext) {
    let (app, mut vcx) = new_app(cx);
    let vcx = &mut vcx;
    let (mute, below) = point_at_mute(&app, vcx);
    move_to(vcx, point(mute.left() + px(10.), below.center().y));
    assert_eq!(submenu(&app, vcx), None);
}
