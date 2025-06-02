use asset_3l14::{AssetFileType, AssetKeySourceId, AssetMetadata, SourceMetadataStub, TomlRead};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use unicase::UniCase;
use crate::core::AssetsBuilderConfig;

#[derive(Debug)]
pub enum ScanError
{
    IOError(io::Error),
    MetaParseError(Box<dyn Error>),
    NoSourceFile
    {
        source_path: PathBuf,
        meta_path: PathBuf,
    },
    NoAssetFile
    {
        asset_path: PathBuf,
        meta_path: PathBuf,
    },
}
impl Error for ScanError { }
impl Display for ScanError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { Debug::fmt(self, f) }
}

pub struct ScanSources
{
    pub(super) walk_dir: walkdir::IntoIter,
}
impl ScanSources
{
    fn read_source_meta(file: impl AsRef<Path>) -> Result<SourceMetadataStub, ScanError>
    {
        let mut fin = File::open(file).map_err(ScanError::IOError)?;
        SourceMetadataStub::load(&mut fin).map_err(|e| ScanError::MetaParseError(e))
    }
}
impl Iterator for ScanSources
{
    type Item = Result<(PathBuf, AssetKeySourceId), ScanError>;

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
                    Some(p) => UniCase::new(p) == AssetsBuilderConfig::SOURCE_META_FILE_EXTENSION
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
                if !source_file.try_exists().map_err(ScanError::IOError).ok()?
                {
                    return Some(Err(ScanError::NoSourceFile
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

pub struct ScanAssets
{
    pub(super) walk_dir: walkdir::IntoIter,
}
impl ScanAssets
{
    fn read_asset_meta(file: impl AsRef<Path>) -> Result<AssetMetadata, ScanError>
    {
        let mut fin = File::open(file).map_err(ScanError::IOError)?;
        AssetMetadata::load(&mut fin).map_err(ScanError::MetaParseError)
    }
}
impl Iterator for ScanAssets
{
    type Item = Result<(PathBuf, AssetMetadata), ScanError>;

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
                    Some(p) => UniCase::new(p) == UniCase::new(AssetFileType::MetaData.file_extension())
                })
                {
                    continue;
                }
                
                // todo: flip this around to find all asset files and then verify meta file

                let asset_meta = match Self::read_asset_meta(entry.path())
                {
                    Ok(am) => am,
                    Err(err) =>
                    {
                        return Some(Err(err));
                    }
                };

                let asset_file = entry.path().with_extension(AssetFileType::Asset.file_extension());
                if !asset_file.try_exists().map_err(ScanError::IOError).ok()?
                {
                    return Some(Err(ScanError::NoAssetFile
                    {
                        asset_path: asset_file,
                        meta_path: entry.into_path(),
                    }));
                }

                return Some(Ok((asset_file, asset_meta)));
            }
        }
        None
    }
}