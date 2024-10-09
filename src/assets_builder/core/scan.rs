use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::path::{Path, PathBuf};
use unicase::UniCase;
use game_3l14::engine::asset::AssetKeySourceId;
use super::{AssetsBuilderConfig, SourceMetadataSlim};

#[derive(Debug)]
pub enum ScanSourcesError
{
    IOError(io::Error),
    MetaParseError(toml::de::Error),
    NoSourceFile
    {
        source_path: PathBuf,
        meta_path: PathBuf,
    },
}
impl Error for ScanSourcesError { }
impl Display for ScanSourcesError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { Debug::fmt(self, f) }
}

pub struct ScanSources
{
    pub(super) walk_dir: walkdir::IntoIter,
}
impl ScanSources
{
    fn read_source_meta(file: impl AsRef<Path>) -> Result<SourceMetadataSlim, ScanSourcesError>
    {
        let source_text = std::fs::read_to_string(file).map_err(ScanSourcesError::IOError)?;
        toml::from_str(&source_text).map_err(ScanSourcesError::MetaParseError)
    }
}
impl Iterator for ScanSources
{
    type Item = Result<(PathBuf, AssetKeySourceId), ScanSourcesError>;

    fn next(&mut self) -> Option<Self::Item>
    {
        while let Some(maybe_entry) = self.walk_dir.next()
        {
            if let Ok(entry) = maybe_entry
            {
                // path.eq_ignore_ascii_case() ? -- use all ascii exts?
                if !entry.path().extension().is_some_and(|ext| match ext.to_str()
                {
                    None => false, // ideally this should be able to compare outside unicode
                    Some(p) => UniCase::new(p) == AssetsBuilderConfig::SOURCE_FILE_META_EXTENSION
                })
                {
                    continue;
                }

                let source_meta = match Self::read_source_meta(entry.path())
                {
                    Ok(sm) => sm,
                    Err(err) =>
                        {
                            return Some(Err(err));
                        }
                };

                // append the extension instead and check that meta file exists?
                // this way checks dangling sources better
                let source_file = entry.path().with_extension("");
                if !source_file.try_exists().map_err(ScanSourcesError::IOError).ok()?
                {
                    return Some(Err(ScanSourcesError::NoSourceFile
                    {
                        source_path: source_file,
                        meta_path: entry.into_path(),
                    }));
                }

                return Some(Ok((source_file, source_meta.source_id)));
            }
        }
        None
    }
}