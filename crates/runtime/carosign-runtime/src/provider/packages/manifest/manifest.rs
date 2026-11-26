use super::check_relative_path;
use super::{
    InstanceModel, ManifestError, PackageSection, ProfileEntry, ProtocolSection, SCHEMA,
    WorkerSection,
};

/// Disassembled manifesto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub schema: u32,
    pub package: PackageSection,
    pub worker: WorkerSection,
    pub protocol: ProtocolSection,
    pub profiles: Vec<ProfileEntry>,
}

/// He's parsing the manifesto.
pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
    let value: toml::Value = text.parse().map_err(|_| ManifestError::Invalid {
        field: "манифест",
        reason: "неверный TOML",
    })?;
    let mut root = table(value, "верхний уровень")?;
    let schema = Some(number(&mut root, "schema", "schema")?);
    let mut p = section(&mut root, "package")?;
    let package = Some(PackageSection {
        id: string(&mut p, "id", "package.id")?,
        version: string(&mut p, "version", "package.version")?,
    });
    no_unknown(p, "package")?;
    let mut w = section(&mut root, "worker")?;
    let executable = string(&mut w, "executable", "worker.executable")?;
    if string(&mut w, "instance_model", "worker.instance_model")? != "per-device" {
        return Err(ManifestError::Invalid {
            field: "worker.instance_model",
            reason: "в v1 допустимо только per-device",
        });
    }
    no_unknown(w, "worker")?;
    let worker = Some(WorkerSection {
        executable,
        instance_model: InstanceModel::PerDevice,
    });
    let mut p = section(&mut root, "protocol")?;
    let protocol = Some(ProtocolSection {
        major: number(&mut p, "major", "protocol.major")?,
        min_minor: number(&mut p, "min_minor", "protocol.min_minor")?,
        max_minor: number(&mut p, "max_minor", "protocol.max_minor")?,
    });
    no_unknown(p, "protocol")?;
    let mut profiles = Vec::new();
    if let Some(value) = root.remove("profiles") {
        let entries = value.as_array().ok_or(ManifestError::Invalid {
            field: "profiles",
            reason: "ожидался массив таблиц",
        })?;
        for entry in entries {
            let mut entry = table(entry.clone(), "profiles")?;
            profiles.push(ProfileEntry {
                id: string(&mut entry, "id", "profiles.id")?,
                path: string(&mut entry, "path", "profiles.path")?,
            });
            no_unknown(entry, "profiles")?;
        }
    }
    no_unknown(root, "верхний уровень")?;
    let schema = schema.ok_or(ManifestError::MissingField { field: "schema" })?;
    if schema != SCHEMA {
        return Err(ManifestError::UnsupportedSchema { found: schema });
    }

    let mut package = package.ok_or(ManifestError::MissingSection { section: "package" })?;
    if package.id.is_empty() {
        return Err(ManifestError::MissingField {
            field: "package.id",
        });
    }
    if package.version.is_empty() {
        return Err(ManifestError::MissingField {
            field: "package.version",
        });
    }
    let name = carosign_core::provider::PackageName::new(&package.id).map_err(|_| {
        ManifestError::Invalid {
            field: "package.id",
            reason: "неверное имя пакета",
        }
    })?;
    package.id = name.as_str().to_owned();
    carosign_core::provider::PackageVersion::new(&package.version).map_err(|_| {
        ManifestError::Invalid {
            field: "package.version",
            reason: "неверная версия пакета",
        }
    })?;

    let worker = worker.ok_or(ManifestError::MissingSection { section: "worker" })?;
    if worker.executable.is_empty() {
        return Err(ManifestError::MissingField {
            field: "worker.executable",
        });
    }
    check_relative_path(&worker.executable)?;
    let protocol = protocol.ok_or(ManifestError::MissingSection {
        section: "protocol",
    })?;
    // Documents 12 and 34: For the current version, it is necessary to be able to agree at
    // least minor 3; an earlier range is not suitable.
    if carosign_protocol::version::negotiate(
        protocol.major,
        protocol.min_minor,
        protocol.max_minor,
    )
    .is_err()
    {
        return Err(ManifestError::Invalid {
            field: "protocol",
            reason: "недопустимый диапазон версии",
        });
    }
    let mut profile_ids = std::collections::BTreeSet::new();
    for profile in &profiles {
        if profile.id.is_empty() || profile.path.is_empty() {
            return Err(ManifestError::Invalid {
                field: "profiles",
                reason: "нет идентификатора или пути",
            });
        }
        check_relative_path(&profile.path)?;
        if !profile_ids.insert(&profile.id) {
            return Err(ManifestError::Duplicate {
                what: "profiles.id".into(),
            });
        }
    }

    Ok(Manifest {
        schema,
        package,
        worker,
        protocol,
        profiles,
    })
}

type TomlTable = toml::map::Map<String, toml::Value>;

fn table(value: toml::Value, field: &'static str) -> Result<TomlTable, ManifestError> {
    value.try_into().map_err(|_| ManifestError::Invalid {
        field,
        reason: "ожидалась таблица",
    })
}

fn section(root: &mut TomlTable, name: &'static str) -> Result<TomlTable, ManifestError> {
    table(
        root.remove(name)
            .ok_or(ManifestError::MissingSection { section: name })?,
        name,
    )
}

fn string(table: &mut TomlTable, key: &str, field: &'static str) -> Result<String, ManifestError> {
    match table
        .remove(key)
        .ok_or(ManifestError::MissingField { field })?
    {
        toml::Value::String(value) => Ok(value),
        _ => Err(ManifestError::Invalid {
            field,
            reason: "ожидалась строка",
        }),
    }
}

fn number(table: &mut TomlTable, key: &str, field: &'static str) -> Result<u32, ManifestError> {
    match table
        .remove(key)
        .ok_or(ManifestError::MissingField { field })?
    {
        toml::Value::Integer(value) => u32::try_from(value).map_err(|_| ManifestError::Invalid {
            field,
            reason: "ожидалось неотрицательное 32-разрядное число",
        }),
        _ => Err(ManifestError::Invalid {
            field,
            reason: "ожидалось число",
        }),
    }
}

fn no_unknown(table: TomlTable, section: &'static str) -> Result<(), ManifestError> {
    if let Some((field, _)) = table.into_iter().next() {
        return Err(ManifestError::UnknownField { section, field });
    }
    Ok(())
}
