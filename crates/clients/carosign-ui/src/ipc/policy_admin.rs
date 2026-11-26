use std::{
    io::{self, Write},
    process::{Command, Output, Stdio},
};

const MAX_POLICY_BYTES: usize = 64 * 1024;

pub(crate) fn read() -> Result<String, &'static str> {
    let result = command("read")
        .output()
        .map_err(|_| "ui-policy-unavailable")?;
    let bytes = response(result)?;
    if bytes.len() > MAX_POLICY_BYTES {
        return Err("ui-policy-unavailable");
    }
    String::from_utf8(bytes).map_err(|_| "ui-policy-unavailable")
}

pub(crate) fn write(expected: &str, proposed: &str) -> Result<(), &'static str> {
    if expected.len() > MAX_POLICY_BYTES || proposed.len() > MAX_POLICY_BYTES {
        return Err("ui-policy-invalid");
    }
    let mut child = command("write")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "ui-policy-unavailable")?;
    let send = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("missing stdin"))
        .and_then(|mut input| {
            for document in [expected, proposed] {
                input.write_all(&(document.len() as u32).to_be_bytes())?;
                input.write_all(document.as_bytes())?;
            }
            Ok(())
        });
    let result = child
        .wait_with_output()
        .map_err(|_| "ui-policy-unavailable")?;
    let response = response(result)?;
    send.map_err(|_| "ui-policy-unavailable")?;
    match response.as_slice() {
        b"saved" => Ok(()),
        b"conflict" => Err("ui-policy-conflict"),
        _ => Err("ui-policy-unavailable"),
    }
}

fn command(operation: &str) -> Command {
    let mut command = Command::new("/usr/bin/pkexec");
    command.args([
        "--disable-internal-agent",
        "/usr/bin/carosign-config",
        "policy-admin",
        operation,
    ]);
    command
}

fn response(output: Output) -> Result<Vec<u8>, &'static str> {
    match output.status.code() {
        Some(0) => Ok(output.stdout),
        Some(126 | 127) => Err("ui-policy-denied"),
        _ => Err("ui-policy-unavailable"),
    }
}
