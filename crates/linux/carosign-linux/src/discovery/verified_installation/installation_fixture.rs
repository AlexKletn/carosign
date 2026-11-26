use std::cell::Cell;
use std::os::unix::fs::PermissionsExt;

use carosign_core::ids::{PackageId, ProfileId, ProviderId};

use crate::sys;

use super::{
    ApprovedFile, ApprovedInstallation, Authority, CheckedFiles, FileRole, InstallationKey,
    Measure, Refusal, checked_files::validate, runtime_decoder::RuntimeDecoder,
};

pub(super) struct Fixture {
    pub(super) root: std::path::PathBuf,
    pub(super) owner: u32,
    pub(super) record: ApprovedInstallation,
    pub(super) revoked: Cell<bool>,
    pub(super) unavailable: Cell<bool>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "carosign-verified-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::create_dir(root.join("profiles")).unwrap();
        std::fs::create_dir(root.join("lib")).unwrap();
        let policy = format!(
            "schema=1\npackage='org.example.provider'\nversion='1.0.0'\nprofile='default'\narchitecture='{}'\nexecutable='worker'\nuid=42\ngid=43\nmemory_bytes=4096\ncpu_quota_us=5000\ncpu_period_us=10000\nprocesses=1\nruntime=['lib/runtime']\nsyscalls=[0,1]\n",
            std::env::consts::ARCH
        );
        std::fs::write(root.join("policy"), policy).unwrap();
        std::fs::write(root.join("package.toml"), "schema=1\n[package]\nid='org.example.provider'\nversion='1.0.0'\n[worker]\nexecutable='worker'\ninstance_model='per-device'\n[protocol]\nmajor=1\nmin_minor=4\nmax_minor=4\n[[profiles]]\nid='default'\npath='profiles/default.toml'\n").unwrap();
        std::fs::write(
            root.join("profiles/default.toml"),
            "id='default'\nmatch.platform='fixture'\n",
        )
        .unwrap();
        std::fs::write(root.join("worker"), b"worker").unwrap();
        std::fs::set_permissions(root.join("worker"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        std::fs::write(root.join("lib/runtime"), b"runtime").unwrap();
        let fd = sys::open_launch_candidate(root.to_str().unwrap()).unwrap();
        let root_metadata = sys::launch_metadata(&fd).unwrap();
        let files = [
            ("policy", FileRole::Policy),
            ("package.toml", FileRole::Manifest),
            ("profiles/default.toml", FileRole::Profile),
            ("worker", FileRole::Executable),
            ("lib/runtime", FileRole::Runtime),
        ]
        .into_iter()
        .map(|(path, role)| {
            (
                path.to_string(),
                ApprovedFile {
                    role,
                    digest: [7; 32],
                },
            )
        })
        .collect();
        Self {
            root,
            owner: root_metadata.identity.uid,
            record: ApprovedInstallation {
                key: InstallationKey {
                    package: PackageId::from_bytes([1; 16]).unwrap(),
                    provider: ProviderId::from_bytes([2; 16]).unwrap(),
                    profile: ProfileId::from_bytes([3; 16]).unwrap(),
                },
                root: root_metadata.identity,
                package: "org.example.provider".into(),
                version: "1.0.0".into(),
                profile: "default".into(),
                version_dir: "version-1".into(),
                signer_key_version: 4,
                signer_fingerprint: vec![8; 20],
                key_policy_id: [9; 16],
                revision: 1,
                policy_path: "policy".into(),
                manifest_path: "package.toml".into(),
                profile_path: "profiles/default.toml".into(),
                executable_path: "worker".into(),
                files,
            },
            revoked: Cell::new(false),
            unavailable: Cell::new(false),
        }
    }
    pub(super) fn run(
        &self,
        key: InstallationKey,
        measure: &dyn Measure,
    ) -> Result<CheckedFiles, Refusal> {
        let root = sys::open_launch_candidate(self.root.to_str().unwrap()).unwrap();
        validate(self, measure, &RuntimeDecoder, key, root, self.owner)
    }
}
impl Authority for Fixture {
    fn approved(&self, _: InstallationKey) -> Result<ApprovedInstallation, Refusal> {
        if self.unavailable.get() {
            return Err(Refusal::Unavailable);
        }
        Ok(self.record.clone())
    }
    fn revalidate(&self, _: &ApprovedInstallation) -> Result<(), Refusal> {
        if self.revoked.get() {
            Err(Refusal::Unavailable)
        } else {
            Ok(())
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
