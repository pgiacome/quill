//! Screenshot demos and fixtures of the Language box: a short official
//! list, a Brazilian Portuguese pack with the strings the box and the
//! Settings list use, and the real Russian overlay, so a switch can be
//! seen without Telegram.

use crate::ui::app::QuillApp;
use crate::ui::screenshot_demo::{DemoSpec, register_demos};
use gpui_kit::*;
use quill::lang::PackValue;
use quill::lang::prefs::LanguageChoice;
use quill::state::LanguageReady;
use quill::telegram::envelope::LanguagePackInfoData;

register_demos![
    DemoSpec::chats("ready-language", "screenshot demo — Settings → Language")
        .setup(QuillApp::demo_language_box),
    DemoSpec::chats(
        "ready-language-pt",
        "screenshot demo — Settings → Language after switching to Português (Brasil)"
    )
    .setup(QuillApp::demo_language_box_pt),
    DemoSpec::chats("ready-language-ru", "screenshot demo — Russian interface")
        .setup(QuillApp::demo_language_box_ru),
];

fn pack(id: &str, name: &str, native: &str, plural: &str, rtl: bool) -> LanguagePackInfoData {
    LanguagePackInfoData {
        id: id.into(),
        base_language_pack_id: String::new(),
        name: name.into(),
        native_name: native.into(),
        plural_code: plural.into(),
        is_official: true,
        is_rtl: rtl,
        is_beta: false,
        is_installed: false,
        total_string_count: 0,
        translated_string_count: 0,
    }
}

/// The official list the demos show.
pub(super) fn demo_packs() -> Vec<LanguagePackInfoData> {
    vec![
        pack("en", "English", "English", "en", false),
        pack(
            "pt-br",
            "Portuguese (Brazil)",
            "Português (Brasil)",
            "pt",
            false,
        ),
        pack("de", "German", "Deutsch", "de", false),
        pack("es", "Spanish", "Español", "es", false),
        pack("fr", "French", "Français", "fr", false),
        pack("it", "Italian", "Italiano", "it", false),
        pack("ru", "Russian", "Русский", "ru", false),
    ]
}

/// The beta packs the demos list, with Telegram's names and counts.
pub(super) fn demo_beta_packs() -> Vec<LanguagePackInfoData> {
    [
        ("ja-raw", "Japanese", "日本語 (beta)", "ja", 8871),
        ("sv-raw", "Swedish", "Svenska (beta)", "sv", 7500),
        ("vi-raw", "Vietnamese", "Tiếng Việt (beta)", "vi", 7794),
    ]
    .into_iter()
    .map(
        |(id, name, native, plural, translated)| LanguagePackInfoData {
            is_official: false,
            is_beta: true,
            total_string_count: 8892,
            translated_string_count: translated,
            ..pack(id, name, native, plural, false)
        },
    )
    .collect()
}

const PT_BR: &[(&str, &str)] = &[
    ("lng_languages", "Idioma"),
    ("lng_settings_language", "Idioma"),
    ("lng_languages_none", "Nenhum idioma encontrado."),
    ("lng_participant_filter", "Pesquisar"),
    ("lng_box_ok", "OK"),
    ("lng_proxy_edit_share", "Compartilhar"),
    ("lng_proxy_menu_delete", "Apagar"),
    ("lng_proxy_menu_restore", "Restaurar"),
    ("lng_translate_settings_show", "Mostrar Botão Traduzir"),
    ("lng_translate_settings_chat", "Traduzir Chats Inteiros"),
    ("lng_translate_settings_choose", "Não Traduzir"),
    ("lng_translate_settings_subtitle", "Traduzir Mensagens"),
    ("lng_menu_settings", "Configurações"),
    ("lng_settings_section_notify", "Notificações e Sons"),
    ("lng_settings_section_privacy", "Privacidade e Segurança"),
    (
        "lng_settings_section_chat_settings",
        "Configurações de Chat",
    ),
    ("lng_settings_advanced", "Avançado"),
];

/// Telegram's Russian values for the box and the Settings list (the `lng_*`
/// strings a live Telegram pack supplies; the `qll_*` half comes from the
/// shipped `lang/ru` overlay).
const RU: &[(&str, &str)] = &[
    ("lng_languages", "Язык"),
    ("lng_settings_language", "Язык"),
    ("lng_languages_none", "Языки не найдены."),
    ("lng_participant_filter", "Поиск"),
    ("lng_box_ok", "ОК"),
    ("lng_translate_settings_show", "Показать кнопку перевода"),
    ("lng_translate_settings_chat", "Переводить чаты целиком"),
    ("lng_translate_settings_choose", "Не переводить"),
    ("lng_translate_settings_subtitle", "Переводить сообщения"),
    ("lng_menu_settings", "Настройки"),
    ("lng_settings_section_notify", "Уведомления и звуки"),
    (
        "lng_settings_section_privacy",
        "Конфиденциальность и безопасность",
    ),
    ("lng_settings_section_chat_settings", "Настройки чатов"),
    ("lng_settings_advanced", "Дополнительно"),
    ("lng_mac_menu_edit", "Правка"),
];

/// A fixture as the pack layer `prefs::build_snapshot` takes.
pub(super) fn fixture_entries(strings: &[(&str, &str)]) -> Vec<(String, PackValue)> {
    strings
        .iter()
        .map(|(key, value)| ((*key).to_string(), PackValue::Ordinary((*value).into())))
        .collect()
}

/// The `lng_*` half of the Russian demo (see `RU`).
pub(super) fn demo_ru_pack() -> Vec<(String, PackValue)> {
    fixture_entries(RU)
}

/// What switching to `info` produces in demos: the Portuguese fixture for
/// `pt-br`, English for anything else.
pub(super) fn demo_ready(info: LanguagePackInfoData) -> LanguageReady {
    let pack = if info.id == "pt-br" {
        fixture_entries(PT_BR)
            .into_iter()
            .chain(std::iter::once((
                "lng_languages_count".to_string(),
                PackValue::Pluralized([
                    String::new(),
                    "{count} idioma".into(),
                    String::new(),
                    String::new(),
                    String::new(),
                    "{count} idiomas".into(),
                ]),
            )))
            .collect()
    } else {
        Vec::new()
    };
    let info = if pack.is_empty() {
        pack_or_english(None)
    } else {
        info
    };
    LanguageReady {
        info,
        pack,
        base: Vec::new(),
    }
}

fn pack_or_english(info: Option<LanguagePackInfoData>) -> LanguagePackInfoData {
    info.unwrap_or_else(|| pack("en", "English", "English", "en", false))
}

impl QuillApp {
    fn demo_language_box(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_language_box(window, cx);
    }

    fn demo_language_box_pt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(info) = demo_packs().into_iter().find(|p| p.id == "pt-br") {
            self.choose_language(info, cx);
        }
        self.open_language_box(window, cx);
    }

    /// The Russian demo: the shipped `qll_*` overlay and the Russian
    /// `lng_*` fixture become the interface's strings, then the Language box
    /// opens on top of them.
    fn demo_language_box_ru(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let choice = LanguageChoice {
            pack_id: "ru".into(),
            plural_code: "ru".into(),
            name: "Russian".into(),
            native_name: "Русский".into(),
            ..Default::default()
        };
        let pack = demo_ru_pack();
        let snapshot = quill::lang::prefs::build_snapshot(&choice, &[], &pack);
        super::language_ui::install_pack(Some(snapshot), cx);
        self.open_language_box(window, cx);
    }
}
