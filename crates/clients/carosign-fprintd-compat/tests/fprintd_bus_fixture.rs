//! Exercises the fprintd interface on an isolated session bus.

use dbus::blocking::Connection;
use dbus::channel::Token;
use dbus::message::MatchRule;
use std::process::Command;
use std::time::Duration;

#[path = "fprintd_bus_fixture/backend_fixture.rs"]
mod backend_fixture;
use backend_fixture::BackendFixture;

#[path = "fprintd_bus_fixture/fixture_backend.rs"]
mod fixture_backend;
use fixture_backend::FixtureBackend;

const MANAGER: &str = "net.reactivated.Fprint.Manager";
const DEVICE: &str = "net.reactivated.Fprint.Device";
const PATH: &str = "/net/reactivated/Fprint/Device/0";

#[test]
fn enrollment_reply_waits_for_preparation_without_blocking_bus() {
    let result = Command::new("dbus-run-session")
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "delayed_enrollment_scenario",
            "--ignored",
            "--nocapture",
        ])
        .output()
        .expect("нужен dbus-run-session");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "запускается на отдельной испытательной шине"]
fn delayed_enrollment_scenario() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };

    let ready = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let (server_tx, server_rx) = mpsc::channel();
    let server_ready = ready.clone();
    let server_stop = stop.clone();
    let server = std::thread::spawn(move || {
        let connection = Connection::new_session().unwrap();
        server_tx.send(()).unwrap();
        carosign_fprintd_compat::dbus::serve_with_backend(
            &connection,
            &mut BackendFixture(server_ready, 0),
            || server_stop.load(Ordering::SeqCst),
        )
        .unwrap();
    });
    server_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let connection = Connection::new_session().unwrap();
        let device = connection.with_proxy("net.reactivated.Fprint", PATH, Duration::from_secs(5));
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            if device
                .method_call::<(), _, _, _>(DEVICE, "Claim", ("",))
                .is_ok()
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
        assert_eq!(device.get::<i32>(DEVICE, "num-enroll-stages").unwrap(), 3);
        let rule = MatchRule::new_signal("org.freedesktop.DBus.Properties", "PropertiesChanged");
        connection.add_match_no_cb(&rule.match_str()).unwrap();
        started_tx.send(()).unwrap();
        let result = device.method_call::<(), _, _, _>(DEVICE, "EnrollStart", ("left-thumb",));
        done_tx.send(result.is_ok()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut stage_count_changed = false;
        let signal = loop {
            let signal = connection
                .channel()
                .blocking_pop_message(deadline.saturating_duration_since(std::time::Instant::now()))
                .unwrap()
                .expect("expected EnrollStatus");
            if signal
                .member()
                .is_some_and(|member| &*member == "PropertiesChanged")
            {
                let (interface, changed, _): (String, dbus::arg::PropMap, Vec<String>) =
                    signal.read3().unwrap();
                if interface == DEVICE {
                    assert_eq!(changed["num-enroll-stages"].0.as_i64(), Some(4));
                    stage_count_changed = true;
                }
            }
            if signal
                .member()
                .is_some_and(|member| &*member == "EnrollStatus")
            {
                break signal;
            }
        };
        assert!(stage_count_changed);
        assert_eq!(
            signal.read2::<String, bool>().unwrap(),
            ("enroll-finger-not-centered".into(), false)
        );
        assert_eq!(device.get::<i32>(DEVICE, "num-enroll-stages").unwrap(), 4);
        let _ = device.method_call::<(), _, _, _>(DEVICE, "EnrollStop", ());
        let _ = device.method_call::<(), _, _, _>(DEVICE, "Release", ());
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        done_rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "ответ пришёл до готовности"
    );
    let observer = Connection::new_session().unwrap();
    let manager = observer.with_proxy(
        "net.reactivated.Fprint",
        "/net/reactivated/Fprint/Manager",
        Duration::from_secs(2),
    );
    let (devices,): (Vec<dbus::Path<'static>>,) =
        manager.method_call(MANAGER, "GetDevices", ()).unwrap();
    assert_eq!(devices.len(), 1, "шина должна отвечать во время подготовки");
    ready.store(true, Ordering::SeqCst);
    assert!(done_rx.recv_timeout(Duration::from_secs(5)).unwrap());
    client.join().unwrap();
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
}

#[test]
fn actual_binary_reports_no_devices_on_private_bus() {
    let executable = std::env::current_exe().unwrap();
    let result = Command::new("dbus-run-session")
        .arg("--")
        .arg(executable)
        .args([
            "--exact",
            "actual_binary_scenario",
            "--ignored",
            "--nocapture",
        ])
        .output()
        .expect("нужен dbus-run-session");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "запускается на отдельной испытательной шине"]
fn actual_binary_scenario() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_carosign-fprintd-compat"))
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
        )
        .env_remove("CAROSIGN_FPRINT_DIAGNOSTICS")
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let connection = Connection::new_session().unwrap();
    let proxy = connection.with_proxy(
        "net.reactivated.Fprint",
        "/net/reactivated/Fprint/Manager",
        Duration::from_millis(200),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let devices: Vec<dbus::Path<'static>> = loop {
        match proxy.method_call::<(Vec<dbus::Path<'static>>,), _, _, _>(MANAGER, "GetDevices", ()) {
            Ok((devices,)) => break devices,
            Err(error) => {
                assert!(std::time::Instant::now() < deadline, "{error}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    assert!(devices.is_empty());
    let duplicate = Command::new(env!("CARGO_BIN_EXE_carosign-fprintd-compat"))
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
        )
        .output()
        .unwrap();
    assert!(
        !duplicate.status.success(),
        "повторный владелец имени запущен"
    );
    let error = proxy
        .method_call::<(dbus::Path<'static>,), _, _, _>(MANAGER, "GetDefaultDevice", ())
        .unwrap_err();
    assert_eq!(
        error.name(),
        Some("net.reactivated.Fprint.Error.NoSuchDevice")
    );
    child.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!String::from_utf8_lossy(&output.stderr).contains("fprintd-diag"));
}

#[test]
fn real_client_over_private_bus() {
    for binary in ["dbus-run-session", "fprintd-verify", "fprintd-enroll"] {
        if Command::new("which")
            .arg(binary)
            .output()
            .map_or(true, |out| !out.status.success())
        {
            eprintln!("испытание пропущено: отсутствует {binary}");
            return;
        }
    }
    let exe = std::env::current_exe().unwrap();
    let result = Command::new("dbus-run-session")
        .arg("--")
        .arg(exe)
        .args([
            "--exact",
            "private_bus_scenario",
            "--ignored",
            "--nocapture",
        ])
        .output()
        .expect("нужен dbus-run-session");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "запускается только на отдельной испытательной шине"]
fn private_bus_scenario() {
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    let done = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&done);
    let enroll_stopped = Arc::new(AtomicBool::new(false));
    let backend_stopped = Arc::clone(&enroll_stopped);
    let stop_ready = Arc::new(AtomicBool::new(true));
    let backend_stop_ready = Arc::clone(&stop_ready);
    let handle = std::thread::spawn(move || {
        let conn = Connection::new_session().unwrap();
        tx.send(()).unwrap();
        carosign_fprintd_compat::dbus::serve_with_backend(
            &conn,
            &mut FixtureBackend {
                enroll_stopped: backend_stopped,
                capture_pending: false,
                stop_ready: backend_stop_ready,
            },
            || stop.load(Ordering::SeqCst),
        )
        .unwrap();
    });
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let client = Connection::new_session().unwrap();
    let proxy = client.with_proxy(
        "net.reactivated.Fprint",
        "/net/reactivated/Fprint/Manager",
        Duration::from_secs(2),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let (device,): (dbus::Path<'static>,) = loop {
        match proxy.method_call(MANAGER, "GetDefaultDevice", ()) {
            Ok(value) => break value,
            Err(error) => {
                assert!(std::time::Instant::now() < deadline, "{error}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    assert_eq!(device.to_string(), PATH);
    let device = client.with_proxy("net.reactivated.Fprint", device, Duration::from_secs(2));
    let name: String = device.get(DEVICE, "name").unwrap();
    assert_eq!(name, "CaroSign");
    // GNOME initializes its proxy with GetAll and lists enrollments before claiming the device.
    let properties = device.get_all(DEVICE).unwrap();
    assert_eq!(
        properties.get("name").and_then(|v| v.0.as_str()),
        Some("CaroSign")
    );
    assert_eq!(
        properties
            .get("num-enroll-stages")
            .and_then(|v| v.0.as_i64()),
        Some(-1)
    );
    let (xml,): (String,) = device
        .method_call("org.freedesktop.DBus.Introspectable", "Introspect", ())
        .unwrap();
    assert!(xml.contains("ListEnrolledFingers"));
    assert!(xml.contains("DeleteEnrolledFingers"));
    assert!(xml.contains("VerifyFingerSelected"));
    let cases = [
        ("NoSuchMember", "org.freedesktop.DBus.Error.UnknownMethod"),
        ("VerifyStart", "org.freedesktop.DBus.Error.InvalidArgs"),
        ("VerifyStop", "net.reactivated.Fprint.Error.ClaimDevice"),
    ];
    for (method, name) in cases {
        let error = device
            .method_call::<(), _, _, _>(DEVICE, method, ())
            .unwrap_err();
        assert_eq!(error.name(), Some(name));
    }
    let error = device.set(DEVICE, "finger-present", true).unwrap_err();
    assert_eq!(
        error.name(),
        Some("org.freedesktop.DBus.Error.PropertyReadOnly")
    );
    let error = device.get::<bool>(DEVICE, "missing").unwrap_err();
    assert_eq!(
        error.name(),
        Some("org.freedesktop.DBus.Error.UnknownProperty")
    );
    let (enrolled,): (Vec<String>,) = device
        .method_call(DEVICE, "ListEnrolledFingers", ("",))
        .unwrap();
    assert_eq!(enrolled, vec!["right-index-finger"]);
    let (tx, rx) = mpsc::channel();
    let rule = MatchRule::new_signal(DEVICE, "VerifyStatus");
    let _: Token = client
        .add_match(rule, move |(result, done): (String, bool), _, _| {
            tx.send((result, done)).unwrap();
            false
        })
        .unwrap();
    device
        .method_call::<(), _, _, _>(DEVICE, "Claim", ("",))
        .unwrap();
    device
        .method_call::<(), _, _, _>(DEVICE, "VerifyStart", ("right-thumb",))
        .unwrap();
    let active = device.get_all(DEVICE).unwrap();
    assert_eq!(active["finger-present"].0.as_i64(), Some(1));
    assert_eq!(active["finger-needed"].0.as_i64(), Some(1));
    device
        .method_call::<(), _, _, _>(DEVICE, "VerifyStop", ())
        .unwrap();
    assert!(!device.get::<bool>(DEVICE, "finger-present").unwrap());
    assert!(!device.get::<bool>(DEVICE, "finger-needed").unwrap());
    device
        .method_call::<(), _, _, _>(DEVICE, "EnrollStart", ("left-thumb",))
        .unwrap();
    device
        .method_call::<(), _, _, _>(DEVICE, "EnrollStop", ())
        .unwrap();
    assert!(enroll_stopped.load(Ordering::SeqCst));

    use dbus::{Message, channel::Sender};
    for next_method in ["EnrollStart", "Release"] {
        device
            .method_call::<(), _, _, _>(DEVICE, "VerifyStart", ("right-thumb",))
            .unwrap();
        stop_ready.store(false, Ordering::SeqCst);
        let request = |member| {
            Message::new_method_call("net.reactivated.Fprint", PATH, DEVICE, member).unwrap()
        };
        let stop_serial = client.send(request("VerifyStop")).unwrap();
        let next = request(next_method);
        let next = if next_method == "EnrollStart" {
            next.append1("left-thumb")
        } else {
            next
        };
        let next_serial = client.send(next).unwrap();
        device.get_all(DEVICE).unwrap();
        stop_ready.store(true, Ordering::SeqCst);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        for expected in [stop_serial, next_serial] {
            loop {
                let mut reply = client
                    .channel()
                    .blocking_pop_message(
                        deadline.saturating_duration_since(std::time::Instant::now()),
                    )
                    .unwrap()
                    .expect("expected reply after stop");
                if let Some(serial) = reply.get_reply_serial() {
                    assert_eq!(serial, expected);
                    reply.as_result().unwrap();
                    break;
                }
            }
        }
        if next_method == "EnrollStart" {
            device
                .method_call::<(), _, _, _>(DEVICE, "EnrollStop", ())
                .unwrap();
        } else {
            device
                .method_call::<(), _, _, _>(DEVICE, "Claim", ("",))
                .unwrap();
        }
    }
    assert!(
        device
            .method_call::<(), _, _, _>(DEVICE, "EnrollStart", ("any",))
            .is_err()
    );
    assert!(
        device
            .method_call::<(), _, _, _>(DEVICE, "DeleteEnrolledFingers2", ())
            .is_err()
    );
    let other = Connection::new_session().unwrap();
    let other_device = other.with_proxy("net.reactivated.Fprint", PATH, Duration::from_secs(2));
    assert!(
        other_device
            .method_call::<(), _, _, _>(DEVICE, "Release", ())
            .is_err()
    );
    let occupied = other_device
        .method_call::<(), _, _, _>(DEVICE, "Claim", ("",))
        .unwrap_err();
    assert_eq!(
        occupied.name(),
        Some("net.reactivated.Fprint.Error.AlreadyInUse")
    );
    device
        .method_call::<(), _, _, _>(DEVICE, "VerifyStart", ("any",))
        .unwrap();
    assert!(
        device
            .method_call::<(), _, _, _>(DEVICE, "EnrollStart", ("left-thumb",))
            .is_err()
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let received = loop {
        if let Ok(value) = rx.try_recv() {
            break value;
        }
        assert!(std::time::Instant::now() < deadline, "сигнал не получен");
        client.process(Duration::from_millis(100)).unwrap();
    };
    assert_eq!(received.0, "verify-no-match");
    assert!(received.1);
    device
        .method_call::<(), _, _, _>(DEVICE, "VerifyStop", ())
        .unwrap();
    device
        .method_call::<(), _, _, _>(DEVICE, "Release", ())
        .unwrap();
    other_device
        .method_call::<(), _, _, _>(DEVICE, "Claim", ("",))
        .unwrap();
    drop(other_device);
    drop(other);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if device
            .method_call::<(), _, _, _>(DEVICE, "Claim", ("",))
            .is_ok()
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "владение после отключения не освобождено"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    device
        .method_call::<(), _, _, _>(DEVICE, "Release", ())
        .unwrap();
    let actual_client = Command::new("fprintd-verify")
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
        )
        .output()
        .expect("нужен установленный fprintd-verify");
    assert!(
        String::from_utf8_lossy(&actual_client.stdout).contains("verify-no-match"),
        "fprintd-verify: {} {}",
        String::from_utf8_lossy(&actual_client.stdout),
        String::from_utf8_lossy(&actual_client.stderr)
    );
    let enroll_client = Command::new("fprintd-enroll")
        .args(["-f", "left-index-finger"])
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
        )
        .output()
        .expect("нужен установленный fprintd-enroll");
    assert!(
        String::from_utf8_lossy(&enroll_client.stdout).contains("enroll-failed"),
        "fprintd-enroll: {} {}",
        String::from_utf8_lossy(&enroll_client.stdout),
        String::from_utf8_lossy(&enroll_client.stderr)
    );
    done.store(true, Ordering::SeqCst);
    handle.join().unwrap();
}
