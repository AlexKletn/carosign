use std::error::Error;
use std::path::PathBuf;

#[path = "build/system_defaults.rs"]
mod system_defaults;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/ui");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing Cargo OUT_DIR")?);
    let styles = "src/ui/shared/styles";
    let options = grass::Options::default().load_path(styles);
    let css = grass::from_path("src/ui/app/window.scss", &options)?;
    std::fs::write(output.join("window.css"), css)?;
    system_defaults::generate(&options, &output.join("system_defaults.rs"))?;
    Ok(())
}
