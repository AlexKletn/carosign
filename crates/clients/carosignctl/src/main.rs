//! Control of the platform from the command line.
//!
//! ⚠️ The shell is the client, not the owner: it decides nothing about authority.

mod socket;

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let (path, words) = if words.first().is_some_and(|word| word == "--socket") {
        if words.len() < 3 || !std::path::Path::new(&words[1]).is_absolute() {
            eprintln!("использование: carosignctl [--socket /абсолютный/путь] команда");
            std::process::exit(2);
        }
        (words[1].as_str(), &words[2..])
    } else {
        ("/run/carosignd/control.sock", &words[..])
    };
    match carosignctl::commands::parse(words) {
        Ok(command) => match socket::execute(&command, path) {
            Ok(lines) => lines.iter().for_each(|line| println!("{line}")),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        },
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
