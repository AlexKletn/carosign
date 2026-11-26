//! The entry point of the broker with private descriptors from the supervisor.

use std::os::fd::{FromRawFd, OwnedFd};

fn id(value: &str) -> Option<[u8; 16]> {
    if value.len() != 32 || !value.is_ascii() {
        return None;
    }
    let mut result = [0; 16];
    for (i, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).ok()?;
    }
    result.iter().any(|b| *b != 0).then_some(result)
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let discovered_worker = args.last().is_some_and(|arg| arg == "--discovered-worker");
    if discovered_worker {
        args.pop();
    }
    let parsed = (|| {
        if args.len() < 12 {
            return None;
        }
        let expected = [
            "--tunnel-fd",
            "--provider-fd",
            "--worker",
            "--epoch",
            "--broker",
            "--boot",
        ];
        if args
            .chunks_exact(2)
            .zip(expected)
            .any(|(pair, key)| pair[0] != key)
        {
            return None;
        }
        let tunnel: i32 = args[1].parse().ok()?;
        let provider: i32 = args[3].parse().ok()?;
        let epoch: u64 = args[7].parse().ok()?;
        if tunnel < 3 || provider < 3 || tunnel == provider || epoch == 0 {
            return None;
        }
        let (owner, next_arg) = if args.get(12).is_some_and(|arg| arg == "--owner-fd") {
            let owner: i32 = args.get(13)?.parse().ok()?;
            if owner < 3 || owner == tunnel || owner == provider {
                return None;
            }
            (Some(owner), 14)
        } else {
            (None, 12)
        };
        let (control, first_resource) =
            if args.get(next_arg).is_some_and(|arg| arg == "--control-fd") {
                let control: i32 = args.get(next_arg + 1)?.parse().ok()?;
                if control < 3 || control == tunnel || control == provider || Some(control) == owner
                {
                    return None;
                }
                (Some(control), next_arg + 2)
            } else {
                (None, next_arg)
            };
        let (owner_managed, first_resource) = if args
            .get(first_resource)
            .is_some_and(|arg| arg == "--owner-managed-resource")
        {
            (Some(id(args.get(first_resource + 1)?)?), first_resource + 2)
        } else {
            (None, first_resource)
        };
        if (args.len() - first_resource) % 6 != 0
            || (args.len() - first_resource) / 6 > carosign_protocol::limits::FD_SLOTS_MAX
        {
            return None;
        }
        let mut resources = Vec::new();
        for entry in args[first_resource..].chunks_exact(6) {
            if entry[0] != "--resource-path"
                || entry[2] != "--resource-id"
                || entry[4] != "--resource-role"
            {
                return None;
            }
            let path = std::path::PathBuf::from(&entry[1]);
            if !path.is_absolute() {
                return None;
            }
            let role = match entry[5].as_str() {
                "device" => 1,
                "config" => 3,
                _ => return None,
            };
            resources.push(carosign_broker::process::ConfiguredResource {
                path,
                resource_id: id(&entry[3])?,
                role,
            });
        }
        Some((
            tunnel,
            provider,
            carosign_broker::process::Context {
                worker: id(&args[5])?,
                epoch,
                broker: id(&args[9])?,
                boot: id(&args[11])?,
            },
            resources,
            owner,
            control,
            owner_managed,
        ))
    })();
    let Some((tunnel, provider, context, resources, owner, control, owner_managed)) = parsed else {
        eprintln!("carosign-broker: не готов: нужны приватные сокеты и контекст supervisor");
        std::process::exit(1);
    };
    // SAFETY: F_GETFD It only checks the existence of the descriptor.
    if unsafe { libc::fcntl(tunnel, libc::F_GETFD) } < 0
        || unsafe { libc::fcntl(provider, libc::F_GETFD) } < 0
        || owner.is_some_and(|fd| unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0)
        || control.is_some_and(|fd| unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0)
    {
        eprintln!("carosign-broker: переданный дескриптор недоступен");
        std::process::exit(1);
    }
    // SAFETY: The supervisor passes two distinct descriptors owned by this process. `PacketSocket`
    // validates each descriptor before use.
    let tunnel = unsafe { OwnedFd::from_raw_fd(tunnel) };
    let provider = unsafe { OwnedFd::from_raw_fd(provider) };
    // SAFETY: Owner, if any, a separate process-owned FD supervisor.
    let owner = owner.map(|fd| unsafe { OwnedFd::from_raw_fd(fd) });
    // SAFETY: A separate descriptor is transmitted to the supervisor and checked above.
    let control = control.map(|fd| unsafe { OwnedFd::from_raw_fd(fd) });
    let run = if discovered_worker {
        carosign_broker::process::run_with_discovered_worker
    } else {
        carosign_broker::process::run_with_control
    };
    if let Err(error) = run(
        tunnel,
        provider,
        context,
        resources,
        owner,
        control,
        owner_managed,
    ) {
        eprintln!("carosign-broker: сеанс завершён: {error:?}");
        std::process::exit(1);
    }
}
