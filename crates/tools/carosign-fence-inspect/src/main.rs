//! One-time reading of the trial base of fences after stopping its owner.
use carosign_persistence::redb::backend::InitialFenceStore;

fn count(path: &std::path::Path) -> Result<usize, Box<dyn std::error::Error>> {
    let store = InitialFenceStore::open_guarded(path)
        .map_err(|error| format!("база ограждений недоступна: {error}"))?;
    let fences = store.fences().map_err(|error| format!("ограждения не прочитаны: {error}"))?;
    Ok(fences.len())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("нужен абсолютный путь к базе")?;
    if args.next().is_some() || !std::path::Path::new(&path).is_absolute() {
        return Err("ожидается один абсолютный путь".into());
    }
    let fences = count(std::path::Path::new(&path))?;
    println!("Оставшихся ограждений: {fences}");
    if fences != 0 {
        return Err("аппаратная выдача остаётся ограждённой".into());
    }
    Ok(())
}

#[path = "main.tests.rs"]
#[cfg(test)]
mod tests;
