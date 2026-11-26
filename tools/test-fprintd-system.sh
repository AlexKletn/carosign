#!/bin/sh
# Exercises the adapter and a separate mock daemon using a private /run.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec unshare --user --map-auto --map-root-user --mount sh -c '
    set -eu
    mount -t tmpfs tmpfs /run
    mkdir -p /run/carosignd
    unset XDG_RUNTIME_DIR
    cd "$1"
    cat > /run/carosignd/bus.conf <<EOF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow user="*"/>
    <allow send_destination="org.freedesktop.DBus"/>
    <allow send_requested_reply="true"/>
    <allow send_type="signal"/>
    <allow receive_sender="*"/>
  </policy>
  <include>$1/packaging/systemd/carosign-fprintd.conf</include>
</busconfig>
EOF
    exec dbus-run-session --config-file=/run/carosignd/bus.conf -- \
        env CAROSIGN_TEST_PRIVATE_RUN=1 cargo test -p carosign-fprintd-compat \
        --test system_backend real_system_backend_scenario -- --ignored --nocapture
' sh "$project_dir"
