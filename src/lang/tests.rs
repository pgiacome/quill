use super::key_table::English;
use super::parse::Form;
use super::*;

fn pack(strings: &[(&str, PackValue)]) -> Snapshot {
    let mut snapshot = Snapshot::new("pt-br", "pt", false);
    snapshot.layer(strings.iter().map(|(k, v)| (*k, v)));
    snapshot
}

fn plural(one: &str, other: &str) -> PackValue {
    PackValue::Pluralized([
        String::new(),
        one.into(),
        String::new(),
        String::new(),
        String::new(),
        other.into(),
    ])
}

#[test]
fn english_is_the_fallback() {
    assert_eq!(text_in(None, keys::LANGUAGES), "Language");
    assert_eq!(english(keys::LANGUAGES), "Language");
    assert_eq!(plural_in(None, keys::LANGUAGES_COUNT, 1, &[]), "1 language");
    assert_eq!(
        plural_in(None, keys::LANGUAGES_COUNT, 3, &[]),
        "3 languages"
    );
    let empty = pack(&[]);
    assert_eq!(text_in(Some(&empty), keys::LANGUAGES), "Language");
}

#[test]
fn pack_values_win_and_unknown_keys_are_ignored() {
    let snapshot = pack(&[
        ("lng_languages", PackValue::Ordinary("Idioma".into())),
        (
            "lng_languages_count",
            plural("{count} idioma", "{count} idiomas"),
        ),
        ("lng_not_used_by_quill", PackValue::Ordinary("x".into())),
    ]);
    assert_eq!(text_in(Some(&snapshot), keys::LANGUAGES), "Idioma");
    // Portuguese: 0 and 1 are "one".
    assert_eq!(
        plural_in(Some(&snapshot), keys::LANGUAGES_COUNT, 0, &[]),
        "0 idioma"
    );
    assert_eq!(
        plural_in(Some(&snapshot), keys::LANGUAGES_COUNT, 2, &[]),
        "2 idiomas"
    );
}

#[test]
fn missing_form_falls_back_to_other_and_kind_mismatch_to_english() {
    let snapshot = pack(&[
        ("lng_languages_count", plural("", "{count} idiomas")),
        ("lng_languages", plural("a", "b")),
    ]);
    assert_eq!(
        plural_in(Some(&snapshot), keys::LANGUAGES_COUNT, 1, &[]),
        "1 idiomas"
    );
    assert_eq!(text_in(Some(&snapshot), keys::LANGUAGES), "Language");
}

#[test]
fn patch_changes_and_deletes() {
    let snapshot = pack(&[
        ("lng_languages", PackValue::Ordinary("Idioma".into())),
        ("lng_box_ok", PackValue::Ordinary("OK!".into())),
    ]);
    let next = snapshot.patched([
        ("lng_languages", &PackValue::Ordinary("Língua".into())),
        ("lng_box_ok", &PackValue::Deleted),
    ]);
    assert_eq!(text_in(Some(&next), keys::LANGUAGES), "Língua");
    assert_eq!(text_in(Some(&next), keys::BOX_OK), "OK");
    // The original is untouched (copy-on-write).
    assert_eq!(text_in(Some(&snapshot), keys::BOX_OK), "OK!");
}

#[test]
fn tags_and_counts() {
    assert_eq!(
        format::substitute("{a} and {b}", &[("a", "x")]),
        "x and {b}"
    );
    assert_eq!(format::substitute("no tags", &[]), "no tags");
    assert_eq!(format::substitute("open {", &[]), "open {");
    assert_eq!(format::group_digits(1234567), "1,234,567");
    assert_eq!(format::group_digits(-1000), "-1,000");
    assert_eq!(format::group_digits(999), "999");
    assert_eq!(format::short_count(9999), "9999");
    assert_eq!(format::short_count(12345), "12.3K");
    assert_eq!(format::short_count(2_000_000), "2M");
    assert_eq!(format::short_count(3_100_000_000), "3.1B");
}

#[test]
fn entries_round_trip_through_a_snapshot() {
    let snapshot = pack(&[
        ("lng_languages", PackValue::Ordinary("Idioma".into())),
        (
            "lng_languages_count",
            plural("{count} idioma", "{count} idiomas"),
        ),
    ]);
    let entries = snapshot.entries();
    assert_eq!(entries.len(), 2);
    let rebuilt = pack(
        &entries
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect::<Vec<_>>(),
    );
    assert_eq!(rebuilt.entries(), entries);
    assert_eq!(snapshot.translated_count("lng_").0, 2);
}

#[test]
fn english_strings_use_the_ellipsis_character() {
    for (name, text) in [
        ("av", include_str!("../../lang/en/av.strings")),
        ("chat", include_str!("../../lang/en/chat.strings")),
        ("media", include_str!("../../lang/en/media.strings")),
        ("money", include_str!("../../lang/en/money.strings")),
        ("ops", include_str!("../../lang/en/ops.strings")),
        ("people", include_str!("../../lang/en/people.strings")),
        ("prefs", include_str!("../../lang/en/prefs.strings")),
        ("search", include_str!("../../lang/en/search.strings")),
        (
            "stories_live",
            include_str!("../../lang/en/stories_live.strings"),
        ),
    ] {
        assert!(
            !text.contains("..."),
            "{name}.strings has a three-dot ellipsis"
        );
    }
}

/// The `{tag}` names of a value, sorted, with repeats.
fn tags(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some((_, tail)) = rest.split_once('{') {
        let Some((name, after)) = tail.split_once('}') else {
            break;
        };
        out.push(name);
        rest = after;
    }
    out.sort_unstable();
    out
}

/// The plural forms a language's rules pick for some whole count.
fn forms_in_use(code: &str) -> std::collections::BTreeSet<usize> {
    let rules = plural::PluralRules::for_code(code);
    (0..1_000).map(|count| rules.form_for(count).index()).collect()
}

/// Every shipped overlay (`lang/<code>/*.strings`) translates only Quill's
/// own English keys, once, with English's placeholders, and gives each
/// plural key every form its language's rules use.
#[test]
fn overlays_match_english() {
    use std::collections::{BTreeMap, BTreeSet};
    // code → base key → the forms given (None for an ordinary key) and
    // their values, across all of the language's files.
    let mut languages: BTreeMap<&str, BTreeMap<String, BTreeMap<Option<usize>, String>>> =
        BTreeMap::new();
    for (code, text) in key_table::OVERLAYS {
        let entries = parse::parse_strings(text).unwrap_or_else(|err| panic!("{code}: {err}"));
        let keys = languages.entry(code).or_default();
        for entry in &entries {
            let (base, form) = parse::split_plural(&entry.key);
            let at = format!("{code}:{}: {}", entry.line, entry.key);
            assert!(base.starts_with("qll_"), "{at}: overlays translate qll_* only");
            assert!(parse::braces_balanced(&entry.value), "{at}: unbalanced braces");
            let index = key_table::index_of(base).unwrap_or_else(|| panic!("{at}: not in English"));
            let english = key_table::ENGLISH.get(index);
            let english_value = match (english, form) {
                (Some(English::Text(value)), None) => Some(*value),
                (Some(English::Plural(forms)), Some(form)) => forms
                    .get(form.index())
                    .copied()
                    .flatten()
                    .or_else(|| forms.get(Form::Other.index()).copied().flatten()),
                _ => panic!("{at}: English has the other kind"),
            };
            if let Some(english_value) = english_value {
                assert_eq!(tags(english_value), tags(&entry.value), "{at}: placeholders");
            }
            let forms = keys.entry(base.to_string()).or_default();
            let previous = forms.insert(form.map(Form::index), entry.value.clone());
            assert!(
                previous.is_none_or(|previous| previous == entry.value),
                "{at}: two different translations"
            );
        }
    }
    for (code, keys) in &languages {
        let needed = forms_in_use(code);
        for (base, forms) in keys {
            if forms.contains_key(&None) {
                continue;
            }
            let given: BTreeSet<usize> = forms.keys().flatten().copied().collect();
            assert!(given.contains(&Form::Other.index()), "{code}: {base} has no #other");
            let missing: Vec<_> = needed.difference(&given).collect();
            assert!(missing.is_empty(), "{code}: {base} lacks forms {missing:?}");
        }
    }
}

#[test]
fn russian_needs_one_few_and_many() {
    use std::collections::BTreeSet;
    // Whole counts never pick Russian's `other` (fractions do); every
    // plural key still needs `#other` as the fallback.
    let ru: BTreeSet<usize> = [Form::One, Form::Few, Form::Many].map(Form::index).into();
    assert_eq!(forms_in_use("ru"), ru);
    assert_eq!(forms_in_use("ja"), [Form::Other.index()].into());
}
