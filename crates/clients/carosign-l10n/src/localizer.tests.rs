use super::*;

fn fixture(locale: &str, style: Option<&str>) -> Localizer {
    Localizer::load(Preferences::new(locale, style), |language, path| {
        let source = match path.to_str()? {
            "base/ru/main.ftl" => include_str!("../../../../locales/base/ru/main.ftl"),
            "styles/w40k/ru/main.ftl" => {
                include_str!("../../../../locales/styles/w40k/ru/main.ftl")
            }
            "styles/broken/ru/main.ftl" => "place-finger = { $missing }",
            _ => return None,
        };
        Catalog::parse(language, source.to_owned())
    })
}

#[test]
fn style_overlays_regional_language_and_missing_keys_use_base() {
    let localizer = fixture("ru_RU.UTF-8", Some("w40k"));
    assert_eq!(
        localizer.text("place-finger"),
        "Приложи палец. Машинный дух ждёт."
    );
    assert_eq!(localizer.text("verify-cancelled"), "Проверка отменена.");
}

#[test]
fn missing_packages_and_invalid_style_identifiers_fall_back() {
    assert_eq!(
        fixture("de-DE", Some("w40k")).text("place-finger"),
        "Place your finger on the reader."
    );
    let preferences = Preferences::new("ru", Some("../../etc"));
    assert!(preferences.style.is_none());
    assert_eq!(
        fixture("ru", Some("broken")).text("place-finger"),
        "Приложите палец к считывателю."
    );
}

#[test]
fn formatted_terminal_message_uses_operation_arguments() {
    let mut args = FluentArgs::new();
    args.set("attempt", 2_u32);
    args.set("seconds", 10_u32);
    assert_eq!(
        fixture("en", None).format("tty-verifying", Some(&args)),
        "Verifying · attempt 2 · 10 s remaining · Enter for password"
    );
}

#[test]
fn pam_or_cli_variables_select_language_and_style_independently() {
    let preferences = Preferences::from_variables(|name| match name {
        "LC_ALL" => Some(String::new()),
        "LC_MESSAGES" => Some("ru_RU.UTF-8".into()),
        "LANG" => Some("en_US.UTF-8".into()),
        "CAROSIGN_STYLE" => Some("w40k".into()),
        _ => None,
    });
    assert_eq!(preferences.locale.to_string(), "ru-RU");
    assert_eq!(preferences.style.as_deref(), Some("w40k"));
}

#[test]
fn every_builtin_style_formats_the_terminal_prompt() {
    let mut args = FluentArgs::new();
    args.set("attempt", 2_u32);
    args.set("total", 3_u32);
    for (language, source) in [
        (
            "en",
            include_str!("../../../../locales/styles/w40k/en/main.ftl"),
        ),
        (
            "ru",
            include_str!("../../../../locales/styles/w40k/ru/main.ftl"),
        ),
        (
            "en",
            include_str!("../../../../locales/styles/tes/en/main.ftl"),
        ),
        (
            "en",
            include_str!("../../../../locales/styles/borg/en/main.ftl"),
        ),
        (
            "ru",
            include_str!("../../../../locales/styles/soviet/ru/main.ftl"),
        ),
    ] {
        let catalog = Catalog::parse(language.parse().unwrap(), source.to_owned()).unwrap();
        assert!(
            catalog
                .format("tty-verifying-continuous", Some(&args))
                .unwrap()
                .contains("2/3")
        );
    }
}
