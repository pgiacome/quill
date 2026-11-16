//! The message field as an element: the kit `Textarea` over the composer's
//! state with what is drawn on it (the inline bot's hint, spelling
//! underlines as text decorations, the words that have an emoji). One state, drawn in one
//! place at a time: the composer bar, or the send box while picked files
//! wait in it, where the field is the caption (`send_card`).

use super::*;
use quill::lang::{keys, tr};

impl QuillApp {
    pub(in crate::ui) fn composer_field(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        // Red wavy underlines under the misspelled words.
        self.sync_spellcheck_decorations(cx);
        div()
            .id("composer-field")
            .relative()
            .flex_1()
            .min_w_0()
            // A field shorter than the buttons beside it is centred on them.
            .when_some(self.density().composer_field_min_h, |field, height| {
                field.min_h(px(height)).flex().items_center()
            })
            .map(|field| self.emoji_words_hover(field, cx))
            // tdesktop hides the field behind a rich draft's preview.
            .when(self.shown_rich_draft().is_some(), |field| field.hidden())
            // Scopes Cmd/Ctrl+K to "edit link" here (see
            // `composer_bindings`).
            .key_context(super::keybindings::COMPOSER_CONTEXT)
            .child(
                Textarea::new(&self.composer)
                    .with_size(self.control_size())
                    .appearance(false)
                    .bordered(false)
                    .aria_label(tr(keys::SR_CHAT_COLUMN_MESSAGE))
                    // Custom emoji drawn inline (codex:composer-input).
                    .token(self.composer_token_renderer(cx))
                    // codex:spellcheck-native: suggestions / Add to
                    // Dictionary / Ignore on a misspelled word.
                    .context_menu({
                        let owner = cx.entity().downgrade();
                        move |_, window, cx| {
                            QuillApp::composer_context_menu(owner.clone(), window, cx)
                        }
                    }),
            )
            // The inline bot's hint after `@bot `.
            .when_some(self.inline_bot_hint_overlay(cx), |wrap, hint| {
                wrap.child(hint)
            })
            // Words that have an emoji.
            .when_some(self.emoji_word_highlights(cx), |wrap, wash| {
                wrap.child(wash)
            })
    }
}
