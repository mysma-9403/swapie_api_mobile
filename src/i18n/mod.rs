use std::collections::HashMap;
use std::sync::OnceLock;

/// In-memory store: language code -> (key -> translation).
type Translations = HashMap<String, HashMap<String, String>>;

static TRANSLATIONS: OnceLock<Translations> = OnceLock::new();

const DEFAULT_LANG: &str = "en";

/// Initialise translations from the embedded JSON files.
///
/// Called once at application startup (e.g. in `main`).  The locale files are
/// embedded at compile time so no filesystem access is needed at runtime.
fn translations() -> &'static Translations {
    TRANSLATIONS.get_or_init(|| {
        let mut map: Translations = HashMap::new();

        // English
        let en_raw = include_str!("../../locales/en.json");
        let en: HashMap<String, String> =
            serde_json::from_str(en_raw).expect("Failed to parse locales/en.json");
        map.insert("en".to_string(), en);

        // Polish
        let pl_raw = include_str!("../../locales/pl.json");
        let pl: HashMap<String, String> =
            serde_json::from_str(pl_raw).expect("Failed to parse locales/pl.json");
        map.insert("pl".to_string(), pl);

        map
    })
}

/// Look up a translation by language and key.
///
/// Falls back to the English translation if the key is missing in the
/// requested language.  Returns the raw key if no translation exists at all.
pub fn t(lang: &str, key: &str) -> String {
    let store = translations();

    // Try requested language first.
    if let Some(lang_map) = store.get(lang) {
        if let Some(value) = lang_map.get(key) {
            return value.clone();
        }
    }

    // Fallback to default language.
    if lang != DEFAULT_LANG {
        if let Some(default_map) = store.get(DEFAULT_LANG) {
            if let Some(value) = default_map.get(key) {
                return value.clone();
            }
        }
    }

    // Nothing found — return the key itself so it is visible in logs / responses.
    key.to_string()
}

/// Look up a translation and replace `{param_name}` placeholders with the
/// supplied values.
///
/// Example:
/// ```ignore
/// let mut params = HashMap::new();
/// params.insert("field".to_string(), "email".to_string());
/// let msg = t_with_params("en", "validation.required", &params);
/// // => "The email field is required"
/// ```
pub fn t_with_params(lang: &str, key: &str, params: &HashMap<String, String>) -> String {
    let mut result = t(lang, key);
    for (param_name, param_value) in params {
        let placeholder = format!("{{{}}}", param_name);
        result = result.replace(&placeholder, param_value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_translation_en() {
        let msg = t("en", "auth.login_success");
        assert_eq!(msg, "Login successful");
    }

    #[test]
    fn test_basic_translation_pl() {
        let msg = t("pl", "auth.login_success");
        assert_eq!(msg, "Logowanie udane");
    }

    #[test]
    fn test_fallback_to_english() {
        // Use a language code that has no translations loaded.
        let msg = t("de", "auth.login_success");
        assert_eq!(msg, "Login successful");
    }

    #[test]
    fn test_missing_key_returns_key() {
        let msg = t("en", "nonexistent.key");
        assert_eq!(msg, "nonexistent.key");
    }

    #[test]
    fn test_params_replacement() {
        let mut params = HashMap::new();
        params.insert("field".to_string(), "email".to_string());
        let msg = t_with_params("en", "validation.required", &params);
        assert_eq!(msg, "The email field is required");
    }

    #[test]
    fn test_params_replacement_multiple() {
        let mut params = HashMap::new();
        params.insert("field".to_string(), "password".to_string());
        params.insert("min".to_string(), "8".to_string());
        let msg = t_with_params("en", "validation.min_length", &params);
        assert_eq!(msg, "The password must be at least 8 characters");
    }
}
