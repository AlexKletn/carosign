use std::{io, path::Path, process::Command};

use super::{SystemAccount, account_icons};

pub(crate) fn read() -> io::Result<Vec<SystemAccount>> {
    let output = Command::new("/usr/bin/getent").arg("passwd").output()?;
    if !output.status.success() {
        return Err(io::Error::other("account enumeration failed"));
    }
    let icons = account_icons::read().unwrap_or_default();
    let mut accounts: Vec<_> = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let mut fields = std::str::from_utf8(line).ok()?.split(':');
            let name = fields.next()?;
            let uid = fields.nth(1)?.parse::<u32>().ok()?;
            let home = Path::new(fields.nth(2)?);
            if name.is_empty() || name.chars().any(char::is_control) {
                return None;
            }
            let avatar = icons
                .get(&uid)
                .filter(|path| is_avatar(path))
                .cloned()
                .or_else(|| {
                    home.is_absolute()
                        .then(|| home.join(".face"))
                        .filter(|path| is_avatar(path))
                });
            Some(SystemAccount {
                name: name.to_owned(),
                uid,
                avatar,
            })
        })
        .collect();
    accounts.sort_unstable_by(|left, right| (&left.name, left.uid).cmp(&(&right.name, right.uid)));
    accounts.dedup_by(|left, right| left.name == right.name && left.uid == right.uid);
    Ok(accounts)
}

fn is_avatar(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|info| info.is_file() && info.len() <= 4 * 1024 * 1024)
}
