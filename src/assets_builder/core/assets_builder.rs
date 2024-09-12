use game_3l14::engine::assets::{AssetKey, AssetKeyBaseId, AssetKeyDerivedId, AssetMetadata, AssetTypeId};
use game_3l14::engine::ShortTypeName;
use rand::RngCore;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use serde::{ser, Serialize, Serializer};
use unicase::UniCase;

use super::*;

struct AssetBuilderEntry
{
    name: &'static str,
    builder: Box<dyn AssetBuilder>,
}

pub struct AssetsBuilderConfig
{
    pub source_files_root: PathBuf,
    pub built_files_root: PathBuf,

    asset_builders: Vec<AssetBuilderEntry>,
    file_ext_to_builder: HashMap<UniCase<&'static str>, usize>,
}
impl AssetsBuilderConfig
{
    pub const SOURCE_FILE_META_EXTENSION: UniCase<&'static str> = UniCase::unicode("sork");

    pub fn new<P: AsRef<Path>>(source_files_root: P, built_files_root: P) -> Self
    {
        Self
        {
            source_files_root: PathBuf::from(source_files_root.as_ref()),
            built_files_root: PathBuf::from(built_files_root.as_ref()),
            asset_builders: Vec::new(),
            file_ext_to_builder: HashMap::new(),
        }
    }

    // Register a builder for it's registered extensions. Will panic if a particular extension was already registered
    pub fn add_builder<B: AssetBuilder>(&mut self, builder: B)
    {
        let b_index = self.asset_builders.len();
        self.asset_builders.push(AssetBuilderEntry
        {
            name: B::short_type_name(),
            builder: Box::new(builder),
        });

        for ext in self.asset_builders[b_index].builder.supported_input_file_extensions()
        {
            if UniCase::new(ext) == Self::SOURCE_FILE_META_EXTENSION
            {
                panic!("Cannot register files as {} as that is a reserved extension", Self::SOURCE_FILE_META_EXTENSION);
            }

            if let Some(obi) = self.file_ext_to_builder.insert(UniCase::new(ext), b_index)
            {
                panic!("Tried to register builder {} for extension {} that was already registered to {}",
                    B::short_type_name(), ext, self.asset_builders[obi].name)
            }
        }
    }
}

pub struct AssetsBuilder
{
    config: AssetsBuilderConfig,
}
impl AssetsBuilder
{
    pub fn new(config: AssetsBuilderConfig) -> Self
    {
        // print errors?
        let _ = std::fs::create_dir_all(&config.built_files_root);
        let _ = std::fs::create_dir_all(&config.source_files_root);

        Self
        {
            config
        }
    }

    // transform a source file into one or more built assets, returns the built count
    pub fn build_assets<P: AsRef<Path> + Debug>(&self, source_path: P) -> Result<BuildResults, BuildError>
    {
        let canonical_path =
        {
            if source_path.as_ref().is_relative()
            {
                self.config.source_files_root.join(source_path.as_ref())
            }
            else
            {
                source_path.as_ref().to_path_buf()
            }
        }.canonicalize().map_err(BuildError::SourceIOError)?;

        let rel_path = canonical_path.strip_prefix(&self.config.source_files_root).map_err(|e| BuildError::InvalidSourcePath)?;

        let file_ext = rel_path.extension().unwrap_or(OsStr::new("")).to_string_lossy();

        let b_index = self.config.file_ext_to_builder.get(&UniCase::from(file_ext.as_ref())).ok_or(BuildError::NoBuilderForSource)?;
        let builder = self.config.asset_builders.get(*b_index).expect("Had builder ID but no matching builder!");

        let source_meta_file_path = canonical_path.with_extension(
            format!("{}.{}", file_ext.as_ref(), AssetsBuilderConfig::SOURCE_FILE_META_EXTENSION));

        let source_meta = match std::fs::File::open(&source_meta_file_path)
        {
            Ok(fin) =>
            {
                ron::de::from_reader(fin).expect("TODO: error handling")
            },
            Err(err) if err.kind() == ErrorKind::NotFound =>
            {
                // TODO: assert that thread_rng impls CryptoRng
                // loop while base ID is zero?
                let mut base_id: AssetKeyBaseId = Default::default();
                rand::thread_rng().fill_bytes(&mut base_id);

                let new_meta = SourceMetadata
                {
                    base_id,
                };

                let meta_write = std::fs::File::create(&source_meta_file_path).map_err(BuildError::SourceMetaIOError)?;
                ron::ser::to_writer_pretty(meta_write, &new_meta, ron::ser::PrettyConfig::new().compact_arrays(true))
                    .map_err(BuildError::SourceMetaSerializeError)?;

                new_meta
            },
            Err(err) =>
            {
                println!("Failed to open source asset meta-file for reading: {err}");
                return Err(BuildError::SourceMetaIOError(err));
            }
        };

        let source_read = std::fs::File::open(&canonical_path).map_err(BuildError::SourceIOError)?;

        let input = SourceInput
        {
            file_extension: UniCase::from(file_ext),
            base_id: source_meta.base_id,
            input: Box::new(source_read),
        };

        let mut outputs = BuildOutputs
        {
            base_id: source_meta.base_id,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            rel_source_path: rel_path,
            abs_output_dir: self.config.built_files_root.as_path(),

            derived_ids: HashMap::new(),
            results: Vec::new(),
        };

        builder.builder.build_assets(input, &mut outputs)?;
        Ok(outputs.results)
    }

    // rebuild_asset(ext, base_id, file_bytes() ?
}

pub type BuildResults = Vec<AssetKey>;

pub struct BuildOutput<W: BuildOutputWrite>
{
    writer: W,
    meta_writer: W,
    asset_key: AssetKey,
    source_path: PathBuf,
    dependencies: Vec<AssetKey>,
}
impl<W: BuildOutputWrite> Write for BuildOutput<W>
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { self.writer.write(buf) } // todo: inline hash?
    fn flush(&mut self) -> std::io::Result<()> { self.writer.flush() }
}
impl<W: BuildOutputWrite> Seek for BuildOutput<W>
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> { self.writer.seek(pos) }
}
impl<W: BuildOutputWrite> BuildOutput<W>
{
    // Serialize some data to the stream using the default serializer
    pub fn serialize<T: Serialize>(&mut self, value: &T) -> Result<(), postcard::Error>
    {
        postcard::to_io(&value, &mut self.writer).map(|_| ())
    }

    pub fn add_dependency(&mut self, dependent_asset: AssetKey)
    {
        self.dependencies.push(dependent_asset);
    }

    pub fn finish(mut self) -> Result<AssetKey, BuildError>
    {
        self.writer.flush().map_err(BuildError::OutputIOError)?;

        self.dependencies.sort();
        self.dependencies.dedup();

        // write metadata
        let asset_meta = AssetMetadata
        {
            key: self.asset_key,
            build_timestamp: chrono::Utc::now().timestamp_millis() as u64,
            source_path: self.source_path,
            dependencies: self.dependencies.into_boxed_slice(),
        };
        // TODO: read old file and compare asset key
        postcard::to_io(&asset_meta, self.meta_writer).map_err(BuildError::OutputSerializeError)?;

        // todo: signal back to BuildOutputs on failure automatically?

        Ok(self.asset_key)
    }
}

pub struct BuildOutputs<'a>
{
    base_id: AssetKeyBaseId,
    timestamp: u64, // unix timestamp in milliseconds

    rel_source_path: &'a Path,
    abs_output_dir: &'a Path,

    derived_ids: HashMap<AssetTypeId, AssetKeyDerivedId>,

    results: BuildResults,

}
impl<'a> BuildOutputs<'a>
{
    // TODO: outputs should be atomic

    // Build one or more outputs from a source. Note: generated asset keys are dependent on call order
    pub fn add_output(&mut self, asset_type: AssetTypeId) -> Result<BuildOutput<impl BuildOutputWrite>, BuildError>
    {
        let derived_id: AssetKeyDerivedId =
        {
            let entry = self.derived_ids.entry(asset_type).or_insert(0);
            entry.checked_add(1).ok_or(BuildError::TooManyDerivedIDs)? - 1
        };

        let asset_key = AssetKey::new(asset_type, derived_id, self.base_id);

        let output_path = self.abs_output_dir.join(asset_key.as_file_name());
        let output_writer = std::fs::File::create(&output_path).map_err(BuildError::OutputIOError)?;

        let output_meta_path = self.abs_output_dir.join(asset_key.as_meta_file_name());
        let output_meta_writer = std::fs::File::create(&output_meta_path).map_err(BuildError::OutputIOError)?;

        let output = BuildOutput
        {
            writer: output_writer,
            meta_writer: output_meta_writer,
            asset_key,
            source_path: self.rel_source_path.to_path_buf(),
            dependencies: Vec::new(),
        };

        self.results.push(asset_key); // TODO: only do if successful?

        Ok(output)
    }
}

pub trait SourceInputRead: Read + Seek { }
impl<T: Read + Seek> SourceInputRead for T { }

pub struct SourceInput
{
    file_extension: UniCase<String>, // does not include .
    base_id: AssetKeyBaseId,
    input: Box<dyn SourceInputRead>, // generic?
}
impl SourceInput
{
    pub fn file_extension(&self) -> &UniCase<String> { &self.file_extension }
}
impl Read for SourceInput
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.input.read(buf) }
}
impl Seek for SourceInput
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> { self.input.seek(pos) }
}
