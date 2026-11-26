//! Entry point for the fprintd-compatible D-Bus adapter.

fn main() {
    if let Err(error) = carosign_fprintd_compat::dbus::serve_system() {
        eprintln!("Служба совместимости fprintd недоступна: {error}");
        std::process::exit(1);
    }
}
