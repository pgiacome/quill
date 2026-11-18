//! Message widgets are off until the user turns them on. A widget is a
//! program a bot sends, and it runs inside Quill: a flaw in the widget
//! engine is a way into Quill and the computer, and such flaws have been
//! found before. While widgets are off no widget's code is loaded
//! (`start_widget` refuses, and the bubble never notes the widget), and the
//! bubble shows a notice with a button that opens the warning. Widgets turn
//! on only from that warning, after its "I understand" box is ticked; the
//! switch in Settings › Privacy and Security opens the same warning, and
//! turns them off at once. The choice is the app's, for every account
//! (`ChatPrefs::message_widgets`). docs/decisions/message-widgets.md.
#![deny(clippy::indexing_slicing)]

use super::app::QuillApp;
use super::message_media::MediaCorners;
use super::shell::{DialogKind, QuillShell};
use super::widgets_state::{Consent, Presence};
use gpui_kit::component::button::*;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::*;
use gpui_kit::*;
use quill::lang::{keys, tr};
use quill::message_widget::Permission;
use std::cell::Cell;
use std::rc::Rc;

impl QuillApp {
    /// Widgets may run: the user turned them on past the warning.
    pub(in crate::ui) fn widgets_allowed(&self) -> bool {
        self.chat_prefs.message_widgets == Permission::On
    }

    /// Open the warning that turns widgets on.
    pub(in crate::ui) fn ask_to_allow_widgets(&mut self, cx: &mut Context<Self>) {
        self.widgets.consent = Consent::Open;
        self.open_box(DialogKind::WidgetConsent);
        cx.notify();
    }

    fn close_widget_consent(&mut self, cx: &mut Context<Self>) {
        // box-host: a close, it opens nothing.
        self.widgets.consent = Consent::Closed;
        cx.notify();
    }

    fn set_widget_consent_understood(&mut self, understood: bool, cx: &mut Context<Self>) {
        if self.widgets.consent == Consent::Closed {
            return;
        }
        // box-host: the open box's own checkbox, where it shows.
        self.widgets.consent = if understood {
            Consent::Understood
        } else {
            Consent::Open
        };
        cx.notify();
    }

    /// The warning's confirm: widgets turn on only once its box is ticked.
    fn allow_widgets(&mut self, cx: &mut Context<Self>) {
        if self.widgets.consent != Consent::Understood {
            return;
        }
        // box-host: its confirm closes it.
        self.widgets.consent = Consent::Closed;
        self.set_chat_prefs(cx, |prefs| prefs.message_widgets = Permission::On);
        cx.notify();
    }

    /// Widgets off: every runtime stops now and the bubbles show the
    /// notice. What the widgets stored is written first and kept, so a
    /// widget turned on again starts from it.
    pub(in crate::ui) fn turn_off_widgets(&mut self, cx: &mut Context<Self>) {
        self.set_chat_prefs(cx, |prefs| prefs.message_widgets = Permission::Off);
        self.save_widget_storage();
        let clock = self.widgets.clock;
        for entry in self.widgets.entries.values_mut() {
            entry.shed();
            entry.presence = Presence::Hidden { since: clock };
            entry.poll = super::widgets_state::Poll::Off;
        }
        self.widgets.poll_timer = None;
        cx.notify();
    }

    fn widget_consent_open(&self) -> bool {
        self.widgets.consent != Consent::Closed
    }

    /// The card a widget message shows while widgets are off: what it is,
    /// and the button to the warning.
    pub(in crate::ui) fn widgets_off_card(
        &self,
        id: u64,
        width: f32,
        cx: &mut Context<Self>,
    ) -> Box<dyn FnOnce(MediaCorners) -> AnyElement> {
        let turn_on = Button::new(("widgets-off-turn-on", id))
            .label(tr(keys::Q_CHAT_WIDGETS_TURN_ON))
            .outline()
            .small()
            .on_click(cx.listener(|this, _, _, cx| this.ask_to_allow_widgets(cx)));
        Box::new(move |_| {
            div()
                .id(("widgets-off", id))
                .debug_selector(|| "widgets-off-card".to_string())
                .w(px(width))
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                // The time's pill sits over the bottom right corner.
                .pb_4()
                .role(Role::Group)
                .aria_label(tr(keys::Q_CHAT_WIDGETS_OFF_TITLE))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            // The bubble's own text color: it reads on an
                            // incoming bubble and an outgoing one alike.
                            Icon::new(IconName::TriangleAlert).small(),
                        )
                        .child(
                            div()
                                .font_semibold()
                                .child(tr(keys::Q_CHAT_WIDGETS_OFF_TITLE)),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .opacity(0.8)
                        .child(tr(keys::Q_CHAT_WIDGETS_OFF_TEXT)),
                )
                .child(
                    div().flex().child(
                        div()
                            .debug_selector(|| "widgets-off-turn-on".to_string())
                            .child(turn_on),
                    ),
                )
                .into_any_element()
        })
    }

    /// Settings › Privacy and Security: the switch. Off turns widgets off
    /// at once; on opens the warning, and the switch shows on only after it.
    pub(in crate::ui) fn message_widgets_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_2()
            .child(div().text_sm().child(tr(keys::Q_CHAT_WIDGETS_SETTING)))
            .child(
                Switch::new("privacy-message-widgets")
                    .checked(self.widgets_allowed())
                    .accessibility_label(tr(keys::Q_CHAT_WIDGETS_SETTING))
                    .on_click(cx.listener(|this, &on: &bool, _, cx| {
                        if on {
                            this.ask_to_allow_widgets(cx);
                        } else {
                            this.turn_off_widgets(cx);
                        }
                    })),
            );
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(self.setting_spot(&["privacy.message_widgets"], row, cx))
            .child(
                div()
                    .text_xs()
                    .px_2()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr(keys::Q_CHAT_WIDGETS_SETTING_ABOUT)),
            )
            .into_any_element()
    }

    fn build_widget_consent_dialog(
        app: &Entity<QuillApp>,
        shell: &Entity<QuillShell>,
        dialog: Dialog,
        cx: &mut App,
    ) -> Dialog {
        let on_close =
            QuillShell::on_close_kind(app, shell, DialogKind::WidgetConsent, |this, _, cx| {
                this.close_widget_consent(cx);
            });
        let understood = app.read(cx).widgets.consent == Consent::Understood;
        app.update(cx, |_, cx| {
            let danger = cx.theme().danger;
            let paragraph = |id: &'static str, key: quill::lang::Key| {
                div()
                    .id(id)
                    .text_sm()
                    .debug_selector(move || id.to_string())
                    .child(tr(key))
            };
            let body = div()
                .id("widget-consent")
                .flex()
                .flex_col()
                .gap_3()
                .pr_6()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_color(danger)
                        .child(Icon::new(IconName::TriangleAlert))
                        .child(
                            div()
                                .font_semibold()
                                .child(tr(keys::Q_CHAT_WIDGETS_WARNING_TITLE)),
                        ),
                )
                .child(paragraph(
                    "widget-consent-what",
                    keys::Q_CHAT_WIDGETS_WARNING_WHAT,
                ))
                .child(
                    paragraph("widget-consent-risk", keys::Q_CHAT_WIDGETS_WARNING_RISK)
                        .font_semibold(),
                )
                .child(paragraph(
                    "widget-consent-advice",
                    keys::Q_CHAT_WIDGETS_WARNING_ADVICE,
                ))
                .child(
                    div()
                        .debug_selector(|| "widget-consent-understand".to_string())
                        .child(
                            Checkbox::new("widget-consent-understand")
                                .label(tr(keys::Q_CHAT_WIDGETS_WARNING_UNDERSTAND))
                                .checked(understood)
                                .on_click(cx.listener(|this, &on: &bool, _, cx| {
                                    this.set_widget_consent_understood(on, cx);
                                })),
                        ),
                )
                .into_any_element();
            let footer = div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    div()
                        .debug_selector(|| "widget-consent-cancel".to_string())
                        .child(
                            Button::new("widget-consent-cancel")
                                .label(tr(keys::CANCEL))
                                .ghost()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.close_widget_consent(cx);
                                    this.close_kit_dialog_if_done(
                                        DialogKind::WidgetConsent,
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                )
                .child(
                    div()
                        .debug_selector(|| "widget-consent-confirm".to_string())
                        .child(
                            Button::new("widget-consent-confirm")
                                .label(tr(keys::Q_CHAT_WIDGETS_WARNING_CONFIRM))
                                .danger()
                                .disabled(!understood)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.allow_widgets(cx);
                                    this.close_kit_dialog_if_done(
                                        DialogKind::WidgetConsent,
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                );
            let body = Rc::new(Cell::new(Some(body)));
            dialog
                .overlay(true)
                .content(crate::ui::shell::scrollable_dialog_content(
                    move |content, _, _| {
                        let body = body.take().unwrap_or_else(|| div().into_any_element());
                        content.child(body)
                    },
                ))
                .footer(footer)
                .on_close(on_close)
        })
    }
}

#[cfg(test)]
#[cfg(feature = "demo-capture")]
#[path = "widget_consent_tests.rs"]
mod tests;

crate::ui::shell::register_dialogs! {
    WidgetConsent => DialogSpec::new(
        3797,
        |app| app.widget_consent_open(),
        QuillApp::build_widget_consent_dialog,
    )
    .close(|app, _, cx| app.close_widget_consent(cx)),
}
