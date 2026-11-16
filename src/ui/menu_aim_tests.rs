//! The menu aim's geometry and stages, on a fixed clock (no waiting).
//!
//! Layout used throughout: a 240 pt menu at x 0..240, its rows 32 pt tall
//! from y 0; the owner row is row 3 (y 96..128). The submenu opens to the
//! right at x 240..480, y 96..320.

use gpui_kit::component::menu::{AimGeometry, AimPointer, SUBMENU_AIM_REST, SubmenuAim};
use gpui_kit::{Bounds, point, px, size};
use std::cell::Cell;
use std::time::{Duration, Instant};

const OWNER: u32 = 3;

fn geometry() -> AimGeometry {
    AimGeometry {
        menu: Bounds::new(point(px(0.), px(0.)), size(px(240.), px(320.))),
        submenu: Bounds::new(point(px(240.), px(96.)), size(px(240.), px(224.))),
    }
}

/// The same menu with its submenu flipped to the left (no room on the right).
fn flipped() -> AimGeometry {
    AimGeometry {
        menu: Bounds::new(point(px(240.), px(0.)), size(px(240.), px(320.))),
        submenu: Bounds::new(point(px(0.), px(96.)), size(px(240.), px(224.))),
    }
}

fn at(x: f32, y: f32, row: Option<u32>) -> AimPointer<u32> {
    AimPointer {
        position: point(px(x), px(y)),
        row,
        pressed: false,
    }
}

/// An aim whose wake-ups are counted, not run.
struct Rig {
    aim: SubmenuAim<u32, ()>,
    wakes: Cell<Vec<Duration>>,
    start: Instant,
}

impl Rig {
    fn new() -> Self {
        Self {
            aim: SubmenuAim::new(),
            wakes: Cell::new(Vec::new()),
            start: Instant::now(),
        }
    }

    fn time(&self, ms: u64) -> Instant {
        self.start + Duration::from_millis(ms)
    }

    fn wakes(&self) -> Vec<Duration> {
        let wakes = self.wakes.take();
        self.wakes.set(wakes.clone());
        wakes
    }

    fn moved(&mut self, ms: u64, pointer: AimPointer<u32>, geometry: AimGeometry) -> bool {
        let now = self.time(ms);
        let wakes = &self.wakes;
        self.aim
            .pointer_moved(Some(OWNER), pointer, Some(geometry), now, |wait| {
                let mut list = wakes.take();
                list.push(wait);
                wakes.set(list);
            })
    }

    fn rested(&mut self, ms: u64, x: f32, y: f32) -> Option<u32> {
        let now = self.time(ms);
        let wakes = &self.wakes;
        self.aim.rested(
            Some(OWNER),
            point(px(x), px(y)),
            Some(geometry()),
            now,
            |wait| {
                let mut list = wakes.take();
                list.push(wait);
                wakes.set(list);
            },
        )
    }
}

#[test]
fn corridor_is_the_triangle_from_the_apex_to_the_near_edge() {
    let g = geometry();
    let apex = point(px(200.), px(112.));
    // Straight down the middle toward the submenu.
    assert!(g.corridor_contains(apex, point(px(220.), px(150.))));
    // Diagonally down, but steeper than the submenu's bottom corner.
    assert!(!g.corridor_contains(apex, point(px(205.), px(200.))));
    // Away from the submenu.
    assert!(!g.corridor_contains(apex, point(px(150.), px(150.))));
    // Above the submenu's top edge.
    assert!(!g.corridor_contains(apex, point(px(230.), px(60.))));
}

#[test]
fn past_the_near_edge_outside_the_menu_the_corridor_is_the_submenu_height() {
    let g = AimGeometry {
        menu: Bounds::new(point(px(0.), px(0.)), size(px(230.), px(320.))),
        ..geometry()
    };
    let apex = point(px(200.), px(112.));
    assert!(g.corridor_contains(apex, point(px(245.), px(300.))));
    assert!(!g.corridor_contains(apex, point(px(245.), px(330.))));
}

#[test]
fn a_submenu_on_the_left_aims_to_its_right_edge() {
    let g = flipped();
    let apex = point(px(280.), px(112.));
    assert!(g.corridor_contains(apex, point(px(260.), px(150.))));
    assert!(!g.corridor_contains(apex, point(px(300.), px(150.))));
}

#[test]
fn unmeasured_submenu_has_no_corridor() {
    let g = AimGeometry {
        submenu: Bounds::default(),
        ..geometry()
    };
    assert!(!g.corridor_contains(point(px(200.), px(112.)), point(px(220.), px(120.))));
}

#[test]
fn a_sibling_on_the_way_to_the_submenu_waits() {
    let mut rig = Rig::new();
    assert!(rig.moved(0, at(200., 112., Some(OWNER)), geometry()));
    // Down-right onto row 4, inside the triangle.
    assert!(!rig.moved(10, at(220., 140., Some(4)), geometry()));
    assert!(rig.aim.holds());
    assert_eq!(rig.wakes(), vec![SUBMENU_AIM_REST]);
    // Keeps moving inside: still waiting, and no second wake-up.
    assert!(!rig.moved(20, at(232., 160., Some(5)), geometry()));
    assert_eq!(rig.wakes().len(), 1);
}

#[test]
fn leaving_the_triangle_hands_the_hover_over_at_once() {
    let mut rig = Rig::new();
    rig.moved(0, at(200., 112., Some(OWNER)), geometry());
    assert!(!rig.moved(10, at(220., 140., Some(4)), geometry()));
    // Turns back left: row 4 takes the hover.
    assert!(rig.moved(20, at(150., 140., Some(4)), geometry()));
    assert!(!rig.aim.holds());
}

#[test]
fn moving_away_from_the_owner_takes_the_hover_at_once() {
    let mut rig = Rig::new();
    rig.moved(0, at(200., 112., Some(OWNER)), geometry());
    assert!(rig.moved(10, at(100., 70., Some(2)), geometry()));
    assert!(rig.wakes().is_empty());
}

#[test]
fn a_drag_never_aims() {
    let mut rig = Rig::new();
    rig.moved(0, at(200., 112., Some(OWNER)), geometry());
    let pressed = AimPointer {
        pressed: true,
        ..at(220., 140., Some(4))
    };
    assert!(rig.moved(10, pressed, geometry()));
}

#[test]
fn resting_inside_the_triangle_lets_the_row_under_take_the_hover() {
    let mut rig = Rig::new();
    rig.moved(0, at(200., 112., Some(OWNER)), geometry());
    rig.moved(10, at(220., 140., Some(4)), geometry());
    // The wake-up fires on time with the pointer still: row 4 wins.
    assert_eq!(rig.rested(310, 220., 140.), Some(4));
    assert!(!rig.aim.holds());
}

#[test]
fn a_move_restarts_the_rest() {
    let mut rig = Rig::new();
    rig.moved(0, at(200., 112., Some(OWNER)), geometry());
    rig.moved(10, at(220., 140., Some(4)), geometry());
    rig.moved(200, at(225., 145., Some(4)), geometry());
    // The first wake-up finds the pointer moved 110 ms ago: it waits on
    // for the rest of the 300 ms, from the stored deadline.
    assert_eq!(rig.rested(310, 225., 145.), None);
    assert!(rig.aim.holds());
    assert_eq!(
        rig.wakes(),
        vec![SUBMENU_AIM_REST, Duration::from_millis(190)]
    );
    assert_eq!(rig.rested(500, 225., 145.), Some(4));
}

#[test]
fn reaching_the_submenu_ends_the_aim_without_a_row() {
    let mut rig = Rig::new();
    rig.moved(0, at(200., 112., Some(OWNER)), geometry());
    rig.moved(10, at(220., 140., Some(4)), geometry());
    assert_eq!(rig.rested(310, 260., 150.), None);
    assert!(!rig.aim.holds());
}

#[test]
fn a_row_left_behind_does_not_take_the_hover() {
    let mut rig = Rig::new();
    rig.moved(0, at(200., 112., Some(OWNER)), geometry());
    rig.moved(10, at(220., 140., Some(4)), geometry());
    rig.aim.row_left(4);
    assert_eq!(rig.rested(310, 238., 140.), None);
}

#[test]
fn back_on_the_owner_the_apex_follows() {
    let mut rig = Rig::new();
    rig.moved(0, at(20., 112., Some(OWNER)), geometry());
    // From far left, (205, 200) is in the triangle...
    assert!(!rig.moved(10, at(205., 200., Some(6)), geometry()));
    rig.moved(20, at(200., 112., Some(OWNER)), geometry());
    // ...but not from the new apex near the edge.
    assert!(rig.moved(30, at(205., 200., Some(6)), geometry()));
}

#[test]
fn a_closed_or_different_submenu_ends_the_aim() {
    let mut rig = Rig::new();
    rig.moved(0, at(200., 112., Some(OWNER)), geometry());
    rig.moved(10, at(220., 140., Some(4)), geometry());
    let now = rig.time(20);
    assert!(
        rig.aim
            .pointer_moved(None, at(220., 141., Some(4)), Some(geometry()), now, |_| ())
    );
    assert!(!rig.aim.holds());
    // A wake-up left over from a submenu that is now another's gives nothing.
    rig.moved(30, at(200., 112., Some(OWNER)), geometry());
    rig.moved(40, at(220., 140., Some(4)), geometry());
    let late = rig.time(400);
    assert_eq!(
        rig.aim.rested(
            Some(9),
            point(px(220.), px(140.)),
            Some(geometry()),
            late,
            |_| ()
        ),
        None
    );
    assert!(!rig.aim.holds());
}

#[test]
fn without_measured_panels_hover_works_as_usual() {
    let mut aim: SubmenuAim<u32, ()> = SubmenuAim::new();
    let now = Instant::now();
    aim.pointer_moved(Some(OWNER), at(200., 112., Some(OWNER)), None, now, |_| ());
    assert!(aim.pointer_moved(Some(OWNER), at(220., 140., Some(4)), None, now, |_| ()));
}
