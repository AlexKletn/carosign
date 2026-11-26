install_sudo_fixture() {
    mkdir -p "$root/etc/pam.d" "$root/usr/lib/security" "$root/usr/lib/sudo" "$root/home/biotest"
    cp -L /usr/bin/sudo /usr/bin/id "$project_dir/target/debug/carosign-fprintd-compat" "$root/usr/bin/"
    cp -L /usr/lib/libdbus-1.so.3 /usr/lib/libsystemd.so.0 "$root/usr/lib/"
    cp -L /usr/lib/sudo/sudoers.so /usr/lib/sudo/libsudo_util.so.0 "$root/usr/lib/sudo/"
    for binary in /usr/bin/sudo /usr/bin/id /usr/lib/sudo/sudoers.so /usr/lib/security/pam_permit.so; do
        ldd "$binary" 2>/dev/null | awk '$2 == "=>" && $3 ~ /^\/usr\/lib\// && $3 !~ /\/sudo\// { print $3 }' |
            while IFS= read -r library; do
                cp -L "$library" "$root/usr/lib/"
            done
    done
    cp -L /usr/lib/security/pam_permit.so "$root/usr/lib/security/"
    cp "$project_dir/target/debug/libpam_carosign.so" "$root/usr/lib/security/pam_carosign.so"
    cc -shared -fPIC -Wall -Wextra -Werror \
        "$project_dir/crates/clients/pam-carosign/tests/fixture_password.c" \
        -o "$root/usr/lib/security/pam_fixture_password.so" -lpam
    printf 'biotest:x:65532:65532:Biometric Test:/home/biotest:/bin/sh\n' >> "$root/etc/passwd"
    printf 'biotest:x:65532:\n' >> "$root/etc/group"
    chown 65532:65532 "$root/home/biotest"
    printf 'uid=0 subject=65532 actions=verify,enroll,remove purpose=session-login\n' \
        >> "$root/etc/carosign/policy.conf"
    chmod 4755 "$root/usr/bin/sudo"
    cat > "$root/etc/sudoers" <<'SUDOERS'
Defaults timestamp_timeout=0, !use_pty, !pam_session, !pam_setcred
biotest ALL=(root) /usr/bin/id -u
SUDOERS
    chmod 440 "$root/etc/sudoers"
    cat > "$root/etc/pam.d/sudo" <<'PAM'
auth sufficient pam_carosign.so
auth required pam_fixture_password.so password
account required pam_permit.so
PAM
}

run_sudo() {
    chroot --userspec=65532:65532 "$root" /usr/bin/sudo -k -S -p '' /usr/bin/id -u
}
