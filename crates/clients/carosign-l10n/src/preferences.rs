use unic_langid::LanguageIdentifier;

pub struct Preferences {
    pub(crate) locale: LanguageIdentifier,
    pub(crate) style: Option<String>,
}

impl Preferences {
    pub fn new(locale: &str, style: Option<&str>) -> Self {
        let normalized = locale
            .split(['.', '@'])
            .next()
            .unwrap_or_default()
            .replace('_', "-");
        let locale = if matches!(normalized.as_str(), "C" | "POSIX") {
            None
        } else {
            normalized.parse().ok()
        };
        let style = style
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 32
                    && *value != "default"
                    && value.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
            })
            .map(str::to_owned);
        Self {
            locale: locale.unwrap_or_else(|| "en".parse().expect("valid fallback locale")),
            style,
        }
    }

    pub fn from_variables(mut variable: impl FnMut(&str) -> Option<String>) -> Self {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| variable(name).filter(|value| !value.is_empty()));
        Self::new(
            locale.as_deref().unwrap_or_default(),
            variable("CAROSIGN_STYLE").as_deref(),
        )
    }

    pub fn from_environment() -> Self {
        Self::from_variables(|name| std::env::var(name).ok())
    }
}
