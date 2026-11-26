use std::path::Path;

use fluent_bundle::FluentArgs;
use unic_langid::LanguageIdentifier;

use super::{Preferences, catalog::Catalog};

const ROOT: &str = "/usr/share/carosign/locales";
const RUSSIAN: &str = include_str!("../../../../locales/base/ru/main.ftl");
const ENGLISH: &str = include_str!("../locales/en.ftl");

pub struct Localizer {
    catalogs: Vec<Catalog>,
}

impl Localizer {
    pub fn system(preferences: Preferences) -> Self {
        let root = Path::new(ROOT);
        Self::load(preferences, |locale, relative| {
            Catalog::installed(locale, &root.join(relative))
        })
    }

    pub fn from_environment() -> Self {
        Self::system(Preferences::from_environment())
    }

    pub fn text(&self, key: &str) -> String {
        self.format(key, None)
    }

    pub fn format(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        self.catalogs
            .iter()
            .find_map(|catalog| catalog.format(key, args))
            .unwrap_or_else(|| key.to_owned())
    }

    fn load(
        preferences: Preferences,
        mut load: impl FnMut(LanguageIdentifier, &Path) -> Option<Catalog>,
    ) -> Self {
        let language = preferences
            .locale
            .language
            .to_string()
            .parse()
            .expect("valid language");
        let mut locales = vec![preferences.locale];
        if !locales.contains(&language) {
            locales.push(language);
        }
        let mut catalogs = Vec::new();
        if let Some(style) = preferences.style {
            for locale in &locales {
                let path = Path::new("styles")
                    .join(&style)
                    .join(locale.to_string())
                    .join("main.ftl");
                catalogs.extend(load(locale.clone(), &path));
            }
        }
        let russian = locales.iter().any(|locale| locale.language.as_str() == "ru");
        for locale in locales {
            let path = Path::new("base").join(locale.to_string()).join("main.ftl");
            catalogs.extend(load(locale, &path));
        }
        if russian {
            catalogs.push(
                Catalog::parse(
                    "ru".parse().expect("valid built-in locale"),
                    RUSSIAN.to_owned(),
                )
                .expect("valid built-in Russian catalog"),
            );
        }
        catalogs.push(
            Catalog::parse(
                "en".parse().expect("valid fallback locale"),
                ENGLISH.to_owned(),
            )
            .expect("valid built-in English catalog"),
        );
        Self { catalogs }
    }
}

#[path = "localizer.tests.rs"]
#[cfg(test)]
mod tests;
