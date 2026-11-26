use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use fluent_bundle::{FluentArgs, FluentResource, concurrent::FluentBundle};
use unic_langid::LanguageIdentifier;

const MAX_CATALOG_BYTES: u64 = 256 * 1024;

pub(crate) struct Catalog(FluentBundle<FluentResource>);

impl Catalog {
    pub(crate) fn parse(locale: LanguageIdentifier, source: String) -> Option<Self> {
        let resource = FluentResource::try_new(source).ok()?;
        let mut bundle = FluentBundle::new_concurrent(vec![locale]);
        bundle.set_use_isolating(false);
        bundle.add_resource(resource).ok()?;
        Some(Self(bundle))
    }

    pub(crate) fn installed(locale: LanguageIdentifier, path: &Path) -> Option<Self> {
        for ancestor in path.parent()?.ancestors() {
            let metadata = ancestor.symlink_metadata().ok()?;
            if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
                return None;
            }
        }
        let metadata = path.symlink_metadata().ok()?;
        if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return None;
        }
        let file = File::open(path).ok()?;
        let mut source = String::new();
        let bytes = file
            .take(MAX_CATALOG_BYTES + 1)
            .read_to_string(&mut source)
            .ok()?;
        if bytes as u64 > MAX_CATALOG_BYTES {
            return None;
        }
        Self::parse(locale, source)
    }

    pub(crate) fn format(&self, key: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
        let pattern = self.0.get_message(key)?.value()?;
        let mut errors = Vec::new();
        let output = self.0.format_pattern(pattern, args, &mut errors);
        (errors.is_empty() && !output.chars().any(char::is_control)).then(|| output.into_owned())
    }
}
