mod configuration;
mod device_admin;
mod diagnostics;
mod policy_admin;

use std::{
    error::Error,
    io::{self, Write},
    path::Path,
    process::ExitCode,
};

use configuration::Configuration;

const USAGE: &str = "Usage: carosign-config [--config /path] show|edit|check service|policy|providers|profile [/path]\n       carosign-config [--config /path] diagnose";

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.as_slice() == ["--help"] {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe) =>
        {
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("carosign-config: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    if let [command, arguments @ ..] = arguments
        && command == "device-admin"
    {
        return device_admin::run(arguments);
    }
    if let [command, arguments @ ..] = arguments
        && command == "policy-admin"
    {
        return policy_admin::run(arguments);
    }
    let (service, args) = match arguments {
        [flag, path, rest @ ..] if flag == "--config" => (Path::new(path), rest),
        _ => (Path::new("/etc/carosign/carosignd.conf"), arguments),
    };
    if !service.is_absolute() {
        return Err("Use an absolute service configuration path".into());
    }
    let configuration = Configuration::new(service);
    match args {
        [command] if command == "diagnose" => diagnostics::diagnose(&configuration),
        [command, target, rest @ ..] if matches!(command.as_str(), "show" | "edit" | "check") => {
            let path = configuration.path(target, rest)?;
            match command.as_str() {
                "show" => io::stdout()
                    .lock()
                    .write_all(configuration::read(&path)?.as_bytes())?,
                "edit" => configuration.edit(target, &path)?,
                _ => {
                    configuration.check(target, &path)?;
                    println!("OK {}", path.display());
                }
            }
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}
