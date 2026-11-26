use std::{error::Error, fmt::Write, path::Path};

pub(super) fn generate(options: &grass::Options, output: &Path) -> Result<(), Box<dyn Error>> {
    let css = grass::from_path("src/ui/shared/styles/system_defaults.scss", options)?;
    let mut source = String::new();
    for block in css
        .trim()
        .split('}')
        .filter(|block| !block.trim().is_empty())
    {
        let (selector, declarations) = block.split_once('{').ok_or("missing defaults block")?;
        let group = selector
            .trim()
            .strip_prefix('.')
            .ok_or("missing defaults group")?;
        let palette = match group {
            "defaults" => None,
            "dark" => Some("DARK"),
            "light" => Some("LIGHT"),
            _ => return Err(format!("unknown defaults group {group}").into()),
        };
        if let Some(constant) = palette {
            writeln!(
                source,
                "const {constant}: FallbackPalette = FallbackPalette {{"
            )?;
        }
        for declaration in declarations
            .split(';')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            let (name, value) = declaration
                .strip_prefix("--")
                .and_then(|item| item.split_once(':'))
                .ok_or("invalid default declaration")?;
            let (kind, literal) = rust_literal(name, value.trim())?;
            let name = name.replace('-', "_");
            if palette.is_some() {
                writeln!(source, "    {name}: {literal},")?;
            } else {
                writeln!(
                    source,
                    "const {}: {kind} = {literal};",
                    name.to_ascii_uppercase()
                )?;
            }
        }
        if palette.is_some() {
            writeln!(source, "}};")?;
        }
    }
    std::fs::write(output, source)?;
    Ok(())
}

fn rust_literal(name: &str, value: &str) -> Result<(&'static str, String), Box<dyn Error>> {
    if name == "color-scheme" {
        let variant = match value {
            "dark" => "Dark",
            "light" => "Light",
            _ => return Err(format!("invalid default color scheme {value}").into()),
        };
        return Ok(("ColorScheme", format!("ColorScheme::{variant}")));
    }
    if let Some(rgb) = value.strip_prefix('#').filter(|rgb| rgb.len() == 6) {
        return Ok(("u32", format!("{:#08x}", u32::from_str_radix(rgb, 16)?)));
    }
    if let Some(family) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Ok(("&str", format!("{family:?}")));
    }
    if let Some(number) = value
        .strip_suffix("pt")
        .or_else(|| value.strip_suffix("px"))
    {
        let number = number.parse::<f32>()?;
        if number.is_finite() {
            return Ok(("f32", format!("{number:?}")));
        }
    }
    Err(format!("invalid system theme default {name}: {value}").into())
}
