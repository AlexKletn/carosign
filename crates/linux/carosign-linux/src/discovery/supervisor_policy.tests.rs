use super::*;
const VALID: &str = "schema=1\npackage='org.example.provider'\nversion='1.0'\nprofile='default'\narchitecture='x86_64'\nexecutable='bin/worker'\nuid=42\ngid=43\nmemory_bytes=4096\ncpu_quota_us=5000\ncpu_period_us=10000\nprocesses=1\nruntime=['lib/a']\nsyscalls=[0,1]\n";
#[test]
fn strict_policy_rejects_ambiguous_and_unsafe_fields() {
    let valid = VALID.replace("x86_64", std::env::consts::ARCH);
    assert!(SupervisorPolicy::parse(valid.as_bytes()).is_ok());
    for bad in [
        format!("{VALID}uid=7\n"),
        format!("{VALID}unknown=1\n"),
        VALID.replace("bin/worker", "../worker"),
        VALID.replace("org.example.provider", "org.-example.provider"),
        VALID.replace("org.example.provider", "org.example_.provider"),
        VALID.replace("org.example.provider", "ORG.example.provider"),
        VALID.replace("uid=42", "uid=0"),
        VALID.replace("[0,1]", "[0,0]"),
        VALID.replace("['lib/a']", "['lib/a','lib/a']"),
        VALID.replace("[0,1]", &format!("[{}]", libc::SYS_socket)),
        VALID.replace("[0,1]", &format!("[{}]", libc::SYS_socketpair)),
        VALID.replace("[0,1]", &format!("[{}]", libc::SYS_io_uring_setup)),
        VALID.replace("[0,1]", &format!("[{}]", libc::SYS_io_uring_enter)),
        VALID.replace("[0,1]", &format!("[{}]", libc::SYS_io_uring_register)),
    ] {
        assert!(SupervisorPolicy::parse(bad.as_bytes()).is_err());
    }
}
