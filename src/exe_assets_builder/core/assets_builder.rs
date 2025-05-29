use std::cell::{LazyCell, UnsafeCell};
use super::*;
use bitcode::Encode;
use metrohash::MetroHash64;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::hash::Hasher;
use std::io;
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use clap::ValueEnum;
use unicase::UniCase;
use asset_3l14::{Asset, AssetFileType, AssetKey, AssetKeyDerivedId, AssetKeySourceId, AssetKeySynthHash, AssetTypeId};
use nab_3l14::utils::inline_hash::InlineWriteHash;
use nab_3l14::utils::{varint, ShortTypeName};
use walkdir::WalkDir;
// TODO: split this file out some?

struct AssetBuilderEntry
{
    name: &'static str,
    builder: Box<dyn ErasedAssetBuilder>,
    builder_hash: BuilderHash,
    format_hash: BuilderHash,
}

#[derive(Debug, Default, Clone, Copy, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum BuildRule
{
    #[default]
    OnlyIfChanged,
    ForceBuildAll,
}

pub struct AssetsBuilderConfig
{
    pub sources_root: PathBuf,
    pub assets_root: PathBuf,
    builders_version_hash: u64,
    asset_builders: Vec<AssetBuilderEntry>,
    file_ext_to_builder: HashMap<UniCase<&'static str>, usize>,
}
impl AssetsBuilderConfig
{
    pub const SOURCE_META_FILE_EXTENSION: UniCase<&'static str> = UniCase::unicode("sork");

    pub fn new<P: Into<PathBuf>>(sources_root: P, assets_root: P) -> Self
    {
        Self
        {
            sources_root: sources_root.into(),
            assets_root: assets_root.into(),
            builders_version_hash: Self::hash_bstrings(0, &[
                b"Initial"
            ]),
            asset_builders: Vec::new(),
            file_ext_to_builder: HashMap::new(),
        }
    }

    pub fn builders_version_hash(&self) -> u64 { self.builders_version_hash }

    fn hash_bstrings(seed: u64, bstrings: &[&[u8]]) -> u64
    {
        let mut hasher = MetroHash64::with_seed(seed);
        bstrings.iter().for_each(|s| { hasher.write(s); });
        hasher.finish()
    }

    // Register a builder for it's registered extensions. Will panic if a particular extension was already registered
    pub fn add_builder<B: AssetBuilder<BuildConfig=impl AssetBuildConfig> + AssetBuilderMeta + 'static>(&mut self, builder: B)
    {
        let b_index = self.asset_builders.len();
        self.asset_builders.push(AssetBuilderEntry
        {
            name: B::short_type_name(),
            builder_hash: BuilderHash(Self::hash_bstrings(self.builders_version_hash, B::builder_version())),
            format_hash: BuilderHash(Self::hash_bstrings(0, B::format_version())),
            builder: Box::new(builder),
        });

        for ext in B::supported_input_file_extensions()
        {
            if UniCase::new(ext) == Self::SOURCE_META_FILE_EXTENSION
            {
                panic!("Cannot register files as {} as that is a reserved extension", Self::SOURCE_META_FILE_EXTENSION);
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
        let _ = std::fs::create_dir_all(&config.assets_root);
        let _ = std::fs::create_dir_all(&config.sources_root);

        Self
        {
            config
        }
    }

    pub fn builders_version_hash(&self) -> u64 { self.config.builders_version_hash }

    pub fn scan_sources(&self) -> ScanSources
    {
        let walker = WalkDir::new(&self.config.sources_root);
        ScanSources { walk_dir: walker.into_iter() }
    }

    pub fn scan_assets(&self) -> ScanAssets
    {
        let walker = WalkDir::new(&self.config.assets_root);
        ScanAssets { walk_dir: walker.into_iter() }
    }

    // transform a source file into one or more built asset, returns the built count
    pub fn build_source<P: AsRef<Path> + Debug>(&self, source_path: P, build_rule: BuildRule) -> Result<BuildResults, BuildError>
    {
        let canonical_path =
        {
            if source_path.as_ref().is_relative()
            {
                self.config.sources_root.join(source_path.as_ref())
            }
            else
            {
                source_path.as_ref().into()
            }
        }.canonicalize().map_err(BuildError::SourceIOError)?;

        let rel_path = canonical_path.strip_prefix(&self.config.sources_root).map_err(|_| BuildError::InvalidSourcePath)?;

        let file_ext = rel_path.extension().unwrap_or(OsStr::new("")).to_string_lossy();

        let b_index = self.config.file_ext_to_builder.get(&UniCase::from(file_ext.as_ref())).ok_or(BuildError::NoBuilderForSource(file_ext.to_string()))?;
        let builder = self.config.asset_builders.get(*b_index).expect("Had builder ID but no matching builder!");

        let source_meta_file_path = canonical_path.with_extension(
            format!("{}.{}", file_ext.as_ref(), AssetsBuilderConfig::SOURCE_META_FILE_EXTENSION));

        let source_meta = match File::open(&source_meta_file_path)
        {
            Ok(mut fin) =>
            {
                let mut meta_contents = String::new();
                fin.read_to_string(&mut meta_contents).map_err(BuildError::SourceMetaIOError)?;
                toml::from_str(&meta_contents).map_err(BuildError::SourceMetaParseError)?
            },
            Err(err) if err.kind() == ErrorKind::NotFound =>
            {
                // TODO: assert that thread_rng impls CryptoRng
                // loop while base ID is zero?
                let source_id = AssetKeySourceId::generate();

                let new_meta = SourceMetadata
                {
                    source_id,
                    build_config: builder.builder.default_config(),
                };

                let meta_string = toml::ser::to_string_pretty(&new_meta).map_err(BuildError::SourceMetaSerializeError)?;
                std::fs::write(&source_meta_file_path, &meta_string).map_err(BuildError::SourceMetaIOError)?;

                new_meta
            },
            Err(err) =>
            {
                log::warn!("Failed to open source asset meta-file for reading: {err}");
                return Err(BuildError::SourceMetaIOError(err));
            }
        };

        let mut source_read =
        {
            let fin = File::open(&canonical_path).map_err(BuildError::SourceIOError)?;
            InlineWriteHash::<MetroHash64, _>::new(Box::new(fin)) // note: seek() makes this hash a bit nondeterministic, but it should be stable as long as the builder/file hasn't changed
        };

        let mut input = SourceInput
        {
            source_path: rel_path,
            file_extension: UniCase::from(file_ext),
            source_id: source_meta.source_id,
            input: &mut source_read,
        };

        let mut outputs = BuildOutputs
        {
            build_rule,
            source_id: source_meta.source_id,
            timestamp: chrono::Utc::now(),
            rel_source_path: rel_path,
            abs_output_dir: self.config.assets_root.as_path(),
            builder_hash: builder.builder_hash,
            format_hash: builder.format_hash,
            derived_ids: HashMap::new(),
            results: HashSet::new(),
        };

        match builder.builder.build_assets(source_meta.build_config, &mut input, &mut outputs)
        {
            Ok(_) =>
            {
                let _input_hash = source_read.finish();
                Ok(outputs.results)
            },
            Err(err) => Err(BuildError::BuilderError(err)),
        }
    }

    // rebuild_asset(ext, base_id, file_bytes() ?
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum BuildError
{
    InvalidSourcePath, // lies outside the sources root
    InvalidSyntheticAssetKey, // asset key was not synthetic
    NoBuilderForSource(String),
    SourceIOError(io::Error),
    SourceMetaIOError(io::Error),
    SourceMetaParseError(toml::de::Error),
    SourceMetaSerializeError(toml::ser::Error),
    TooManyDerivedIDs,
    BuilderError(Box<dyn Error>),
    OutputIOError(io::Error),
    OutputMetaIOError(io::Error),
    OutputDebugIOError(io::ErrorKind), // error kind b/c error is not cloneable and lazy makes this stupid
    OutputMetaSerializeError(toml::ser::Error),
}
impl Display for BuildError
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { std::fmt::Debug::fmt(&self, f) }
}
impl Error for BuildError { }

pub type BuildResults = HashSet<AssetKey>; // TODO: IndexSet

struct Lazy<T, F: FnOnce() -> T>
{
    value: UnsafeCell<Option<T>>,
    create_fn: MaybeUninit<F>,
}
impl<T, F: FnOnce() -> T> Lazy<T, F>
{
    pub fn new(create_fn: F) -> Self
    {
        Self { value: UnsafeCell::new(None), create_fn: MaybeUninit::new(create_fn) }
    }
    fn force(&self) -> &mut T
    {
        let val = unsafe { &mut *self.value.get() };
        match val
        {
            None => unsafe
            {
                let create_fn = self.create_fn.assume_init_read();
                let _ = std::mem::replace(val, Some(create_fn()));
                val.as_mut().unwrap_unchecked()
            }
            Some(val) => val
        }
    }
}
impl<T, F: FnOnce() -> T> Deref for Lazy<T, F>
{
    type Target = T;

    fn deref(&self) -> &Self::Target
    {
        self.force()
    }
}
impl<T, F: FnOnce() -> T> DerefMut for Lazy<T, F>
{
    fn deref_mut(&mut self) -> &mut Self::Target
    {
        self.force()
    }
}

pub struct BuildOutput
{
    writer: Box<dyn BuildOutputWrite>,
    meta_writer: Box<dyn BuildOutputWrite>,
    debug_data_file_path: PathBuf,
    builder_hash: BuilderHash,
    format_hash: BuilderHash,
    asset_key: AssetKey,
    name: Option<String>,
    source_path: PathBuf,
    dependencies: Vec<AssetKey>,
}
impl Write for BuildOutput
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.writer.write(buf) } // todo: inline hash?
    fn flush(&mut self) -> io::Result<()> { self.writer.flush() }
}
impl Seek for BuildOutput
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> { self.writer.seek(pos) }
}
impl BuildOutput
{
    pub fn asset_key(&self) -> AssetKey { self.asset_key }

    /* TODO: use savefile for serialization?
    - versioned, can do migrations more easily
        migrations would take the form of loading the old asset, applying any transforms, and re-baking
    - useful?
    - type hashing (type_hash)?
     */

    pub fn set_name(&mut self, name: impl Into<String>) { self.name = Some(name.into()); }

    pub fn depends_on(&mut self, dependency: AssetKey)
    {
        self.dependencies.push(dependency);
    }
    // TODO: clean up
    pub fn depends_on_multiple(&mut self, dependencies: impl IntoIterator<Item=AssetKey>)
    {
        self.dependencies.extend(dependencies)
    }

    // Serialize some size-prefixed data to the stream using the default serializer, writes all or nothing
    pub fn serialize<T: Encode>(&mut self, value: &T) -> io::Result<()>
    {
        let val = bitcode::encode(value);
        varint::encode_into(val.len() as u64, &mut self.writer)?;
        self.writer.write_all(val.as_slice())
    }

    // Optionally serialize debug metadata to this asset
    pub fn serialize_debug<A: Asset>(&mut self, value: &A::DebugData) -> io::Result<()>
    {
        let mut debug_writer = File::create(&self.debug_data_file_path)?;

        let val = bitcode::encode(value);
        varint::encode_into(val.len() as u64, &mut debug_writer)?;
        debug_writer.write_all(val.as_slice())
    }

    // write a size-prefixed span of bytes, all or nothing
    pub fn write_sized(&mut self, buf: &[u8]) -> Result<(), impl Error>
    {
        varint::encode_into(buf.len() as u64, &mut self.writer)?;
        self.writer.write_all(buf)
    }

    fn finish(mut self) -> Result<AssetKey, BuildError>
    {
        self.writer.flush().map_err(BuildError::OutputIOError)?;

        self.dependencies.dedup();

        // TODO: this can be pulled back into BuildOutputs

        // write metadata
        let asset_meta = AssetMetadata
        {
            key: self.asset_key,
            name: self.name,
            source_path: self.source_path,
            build_timestamp: chrono::Utc::now(),
            builder_hash: self.builder_hash,
            format_hash: self.format_hash,
            dependencies: self.dependencies.into_boxed_slice(),
        };
        // TODO: read old file and compare asset key

        let out_string = toml::ser::to_string(&asset_meta).unwrap();//.map_err(BuildError::OutputMetaSerializeError)?;
        self.meta_writer.write_all(out_string.as_bytes()).map_err(BuildError::OutputMetaIOError)?;

        Ok(self.asset_key)
    }
}

pub struct BuildOutputs<'b>
{
    build_rule: BuildRule,
    source_id: AssetKeySourceId,
    timestamp: chrono::DateTime<chrono::Utc>,

    rel_source_path: &'b Path,
    abs_output_dir: &'b Path,

    builder_hash: BuilderHash,
    format_hash: BuilderHash,

    derived_ids: HashMap<AssetTypeId, AssetKeyDerivedId>,

    results: BuildResults,
}
impl<'b> BuildOutputs<'b>
{
    // TODO: outputs should be atomic (all or none)

    #[inline] #[must_use]
    pub fn source_path(&self) -> &Path { self.rel_source_path }


    // Produce an output from this build. Assets of the same type have sequential derived IDs
    #[inline]
    pub fn add_output(
        &mut self,
        asset_type: AssetTypeId,
        builder_fn: impl FnOnce(&mut BuildOutput) -> Result<(), Box<dyn Error>>)
        -> Result<AssetKey, BuildError>
    {
        let derived_id: AssetKeyDerivedId =
        {
            let entry = self.derived_ids.entry(asset_type).or_insert(AssetKeyDerivedId::default());
            entry.next().ok_or(BuildError::TooManyDerivedIDs)?
        };

        let asset_key = AssetKey::unique(asset_type, derived_id, self.source_id);
        self.add_asset(asset_key, builder_fn)
    }

    // Produce an output from ths build that is referenced by a calculable hash. By default, will only return an output if the hash doesn't already exist
    #[inline]
    pub fn add_synthetic(
        &mut self,
        asset_type: AssetTypeId,
        asset_hash: AssetKeySynthHash,
        builder_fn: impl FnOnce(&mut BuildOutput) -> Result<(), Box<dyn Error>>)
        -> Result<AssetKey, BuildError>
    {
        let asset_key = AssetKey::synthetic(asset_type, asset_hash);
        self.add_asset(asset_key, builder_fn)
    }

    // build an asset (if rules allow) and add an output to the asset build
    fn add_asset(
        &mut self, asset_key: AssetKey, builder_fn: impl FnOnce(&mut BuildOutput) -> Result<(), Box<dyn Error>>)
        -> Result<AssetKey, BuildError>
    {
        let output_path = self.abs_output_dir.join(asset_key.as_file_name(AssetFileType::Asset));

        let should_build = match self.build_rule
        {
            BuildRule::OnlyIfChanged =>
            {
                !output_path.exists()
                // TODO: check if actually different (only for synthetic assets)
            },
            BuildRule::ForceBuildAll => true,
        };
        if should_build && !self.results.contains(&asset_key)
        {
            let output_writer = File::create(&output_path).map_err(BuildError::OutputIOError)?;

            let output_meta_path = self.abs_output_dir.join(asset_key.as_file_name(AssetFileType::MetaData));
            let output_meta_writer = File::create(&output_meta_path).map_err(BuildError::OutputIOError)?;

            let output_debug_path = self.abs_output_dir.join(asset_key.as_file_name(AssetFileType::DebugData));

            let mut output = BuildOutput
            {
                writer: Box::new(output_writer),
                meta_writer: Box::new(output_meta_writer),
                debug_data_file_path: output_debug_path,
                builder_hash: self.builder_hash,
                format_hash: self.format_hash,
                asset_key,
                name: None,
                source_path: self.rel_source_path.to_path_buf(),
                dependencies: Vec::new(),
            };

            log::debug!("Building asset {}", asset_key);
            builder_fn(&mut output).map_err(BuildError::BuilderError)?;
            output.finish()?;
        }

        self.results.insert(asset_key);
        Ok(asset_key)
    }
}

pub trait SourceInputRead: Read + Seek { }
impl<T: Read + Seek> SourceInputRead for T { }

pub struct SourceInput<'b>
{
    source_path: &'b Path, // Should only be used for debug purposes
    file_extension: UniCase<String>, // does not include .
    source_id: AssetKeySourceId,
    input: &'b mut dyn SourceInputRead,
}
impl<'b> SourceInput<'b>
{
    pub fn source_path_string(&self) -> String { self.source_path.to_string_lossy().to_string() }
    pub fn file_extension(&self) -> &UniCase<String> { &self.file_extension }
}
impl<'b> Read for SourceInput<'b>
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.input.read(buf) }
}
impl<'b> Seek for SourceInput<'b>
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> { self.input.seek(pos) }
}
// todo: