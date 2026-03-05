use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

use fluent_bundle::FluentValue;
use fluent_templates::{Loader, static_loader};
use tracing::warn;
use unic_langid::LanguageIdentifier;

pub(crate) const FALLBACK_LOCALE: &str = "en-US";
pub(crate) const SUPPORTED_LOCALES: &[&str] = &[
    "en-US", "el-GR", "zh-CN", "de-DE", "es-ES", "pt-BR", "ko-KR", "ja-JP", "fr-FR",
];

static_loader! {
    static LOCALES = {
        locales: "./src/i18n/locales",
        fallback_language: "en-US",
    };
}

pub(crate) trait Localizer: Send + Sync {
    fn text(&self, locale: &str, key: &str) -> String;
    fn text_with_vars(&self, locale: &str, key: &str, vars: &[(&str, String)]) -> String;
}

#[derive(Default)]
pub(crate) struct FluentLocalizer;

impl Localizer for FluentLocalizer {
    fn text(&self, locale: &str, key: &str) -> String {
        let language = parse_locale(locale);
        let value = LOCALES.lookup(&language, key);
        if value == key {
            warn!(locale, key, "i18n.missing_key");
        }
        value
    }

    fn text_with_vars(&self, locale: &str, key: &str, vars: &[(&str, String)]) -> String {
        let language = parse_locale(locale);
        let mut args: HashMap<Cow<'static, str>, FluentValue<'static>> = HashMap::new();
        for &(name, ref value) in vars {
            args.insert(
                Cow::Owned(name.to_owned()),
                FluentValue::from(value.clone()),
            );
        }
        let value = LOCALES.lookup_with_args(&language, key, &args);
        if value == key {
            warn!(locale, key, "i18n.missing_key");
        }
        value
    }
}

static GLOBAL_LOCALIZER: LazyLock<FluentLocalizer> = LazyLock::new(FluentLocalizer::default);

pub(crate) fn t(locale: &str, key: &str) -> String {
    GLOBAL_LOCALIZER.text(locale, key)
}

pub(crate) fn t_vars(locale: &str, key: &str, vars: &[(&str, String)]) -> String {
    GLOBAL_LOCALIZER.text_with_vars(locale, key, vars)
}

pub(crate) fn resolve_supported_locale(raw_locale: &str) -> String {
    let normalized = normalize_locale_tag(raw_locale);
    if SUPPORTED_LOCALES.iter().any(|&l| l == normalized) {
        return normalized;
    }

    let lang = normalized.split('-').next().unwrap_or_default();
    if let Some(found) = SUPPORTED_LOCALES
        .iter()
        .find(|l| l.starts_with(&format!("{lang}-")))
    {
        return (*found).to_string();
    }

    FALLBACK_LOCALE.to_string()
}

fn parse_locale(raw_locale: &str) -> LanguageIdentifier {
    let resolved = resolve_supported_locale(raw_locale);
    if let Ok(id) = resolved.parse::<LanguageIdentifier>() {
        id
    } else {
        warn!(locale = %resolved, "i18n.invalid_locale_fallback");
        FALLBACK_LOCALE
            .parse::<LanguageIdentifier>()
            .unwrap_or_default()
    }
}

fn normalize_locale_tag(raw_locale: &str) -> String {
    raw_locale.trim().replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::{FALLBACK_LOCALE, resolve_supported_locale, t};

    #[test]
    fn resolves_exact_supported_locale() {
        assert_eq!(resolve_supported_locale("zh-CN"), "zh-CN");
    }

    #[test]
    fn resolves_by_language_prefix() {
        assert_eq!(resolve_supported_locale("zh"), "zh-CN");
    }

    #[test]
    fn falls_back_for_unknown_locale() {
        assert_eq!(resolve_supported_locale("xx-YY"), FALLBACK_LOCALE);
    }

    #[test]
    fn returns_localized_text_for_known_key() {
        let value = t("en-US", "response-user-context-header");
        assert_eq!(value, "Recent activity context:");
    }
}
