use std::{fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt, path::Path, sync::Arc};

use obscur_kit::resource_decode::decoder::ImageDecoder;
use obscur_kit::resources::port::{LoadedBytes, ResourceLoader};
use obscur_kit::resources::source::ResourceSource;
use obscur_kit::resources::state::{ResourceError, ResourceMetadata};
use obscur_kit::{AppControl, bootstrap::KitBootstrap, system_platform::icons::theme_icon_svg};

pub(in crate::ui) struct WidgetImages;

impl WidgetImages {
    pub(in crate::ui) fn install(control: &AppControl) {
        control.with_runtime(|runtime| {
            runtime.set_resource_providers(Arc::new(Self), Arc::new(ImageDecoder::new()));
            KitBootstrap::new().install_vector_resources(runtime);
        });
    }

    fn bytes(source: &ResourceSource) -> Option<&'static [u8]> {
        let ResourceSource::File(path) = source else {
            return None;
        };
        match path.as_ref() {
            "chevron-left.svg" => Some(include_bytes!("../app/dashboard/icons/chevron-left.svg")),
            "window-minimize.svg" => Some(include_bytes!("../app/window_icons/minimize.svg")),
            "window-maximize.svg" => Some(include_bytes!("../app/window_icons/maximize.svg")),
            "window-close.svg" => Some(include_bytes!("../app/window_icons/close.svg")),
            "window-restore.svg" => Some(include_bytes!("../app/window_icons/restore.svg")),
            "platform.svg" => Some(include_bytes!("sidebar/icons/platform.svg")),
            "restart.svg" => Some(include_bytes!("sidebar/icons/restart.svg")),
            "policies.svg" => Some(include_bytes!("sidebar/icons/policies.svg")),
            "fingerprint.svg" => Some(include_bytes!("sidebar/icons/fingerprint.svg")),
            "face.svg" => Some(include_bytes!("sidebar/icons/face.svg")),
            "iris.svg" => Some(include_bytes!("sidebar/icons/iris.svg")),
            "voice.svg" => Some(include_bytes!("sidebar/icons/voice.svg")),
            "fingerprint-chevron-down.svg" => {
                Some(include_bytes!("fingerprints/images/chevron-down.svg"))
            }
            "fingerprint-chevron-up.svg" => {
                Some(include_bytes!("fingerprints/images/chevron-up.svg"))
            }
            _ => None,
        }
    }

    fn themed_icon(source: &ResourceSource) -> Option<(&str, &str, u16, &str)> {
        let ResourceSource::File(key) = source else {
            return None;
        };
        let mut parts = key.strip_prefix("window-theme-icon/")?.split('/');
        let scale = parts.next()?.parse().ok()?;
        let theme = parts.next()?;
        let name = parts.next()?;
        let fallback = parts.next()?;
        parts
            .next()
            .is_none()
            .then_some((theme, name, scale, fallback))
    }

    fn avatar_path(source: &ResourceSource) -> Option<&Path> {
        let ResourceSource::File(key) = source else {
            return None;
        };
        let path = Path::new(key.strip_prefix("account-avatar:")?);
        path.is_absolute().then_some(path)
    }

    fn avatar_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
        const LIMIT: u64 = 4 * 1024 * 1024;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > LIMIT {
            return Err(std::io::Error::other("invalid avatar file"));
        }
        let mut bytes = Vec::new();
        file.take(LIMIT + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > LIMIT {
            return Err(std::io::Error::other("avatar is too large"));
        }
        Ok(bytes)
    }
}

impl ResourceLoader for WidgetImages {
    fn can_load(&self, source: &ResourceSource) -> bool {
        Self::bytes(source).is_some()
            || Self::themed_icon(source).is_some()
            || Self::avatar_path(source).is_some()
    }

    fn load(&self, source: &ResourceSource) -> Result<LoadedBytes, ResourceError> {
        let bytes: Arc<[u8]> = if let Some(bytes) = Self::bytes(source) {
            Arc::from(bytes)
        } else if let Some((theme, name, scale, fallback)) = Self::themed_icon(source) {
            theme_icon_svg(theme, name, 16, scale)
                .map(Arc::from)
                .or_else(|| Self::bytes(&ResourceSource::file(fallback)).map(Arc::from))
                .ok_or_else(|| ResourceError::SourceUnavailable("window icon unavailable".into()))?
        } else {
            let path = Self::avatar_path(source)
                .ok_or_else(|| ResourceError::SourceUnavailable("unknown image".into()))?;
            Self::avatar_bytes(path).map(Arc::from).map_err(|_| {
                ResourceError::SourceUnavailable("account avatar unavailable".into())
            })?
        };
        let metadata = ResourceMetadata::default().with_byte_size(bytes.len() as u64);
        Ok(LoadedBytes::new(bytes, metadata))
    }
}
