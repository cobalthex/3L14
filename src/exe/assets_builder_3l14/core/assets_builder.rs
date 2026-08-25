use super::*;
use asset_3l14::{Asset, AssetFileType, AssetKey, AssetKeyDerivedId, AssetKeySourceId, AssetKeySynthHash, AssetMetadata, AssetTypeId, VersionHash, SourceMetadata, TomlRead, TomlWrite, SourceMetadataStub, MetaFileError, AssetLoadError};
use bitcode::Encode;
use clap::ValueEnum;
use metrohash::MetroHash64;
use nab_3l14::utils::inline_hash::InlineWriteHash;
use nab_3l14::utils::{varint, ShortTypeName, val_as_u8_slice};
use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io;
use std::io::{Cursor, ErrorKind, Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use dashmap::DashMap;
use unicase::UniCase;
use walkdir::WalkDir;
use nab_3l14::Symbol;
// TODO: split this file out some?

struct AssetBuilderEntry
{
    name: &'static str,
    builder: Box<dyn ErasedAssetBuilder>,
    version_hash: VersionHash,
}

#[derive(Debug, Default, Clone, Copy, ValueEnum)]
#[clap()]
pub enum BuildRule
{
    #[default]
    #[value(alias = "changed")]
    OnlyIfChanged,
    #[value(alias = "all")]
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
    pub const SOURCE_META_FILE_EXTENSION: UniCase<&'static str> = UniCase::unicode("sork"); // TODO: OsStr?

    pub fn new<P: Into<PathBuf>>(sources_root: P, assets_root: P) -> Self
    {
        Self
        {
            sources_root: sources_root.into(),
            assets_root: assets_root.into(),
            builders_version_hash: {
                let mut vb = VersionBuilder::new(0);
                vb.append(&[
                    b"Initial"
                ]);
                vb.build_raw()
            },
            asset_builders: Vec::new(),
            file_ext_to_builder: HashMap::new(),
        }
    }

    pub fn builders_version_hash(&self) -> u64 { self.builders_version_hash }

    // Register a builder for it's registered extensions. Will panic if a particular extension was already registered
    pub fn add_builder<B: AssetBuilder<BuildConfig=impl AssetBuildConfig> + 'static>(&mut self, builder: B)
    {
        let mut versioner = VersionBuilder::new(self.builders_version_hash);
        builder.format_version(&mut versioner);
        builder.builder_version(&mut versioner);

        let supported_exts = builder.supported_input_file_extensions();

        let b_index = self.asset_builders.len();
        self.asset_builders.push(AssetBuilderEntry
        {
            name: B::short_type_name(),
            version_hash: versioner.build(),
            builder: Box::new(builder),
        });

        for ext in supported_exts
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
    // TODO: use Path -- and make case insensitive?
    sources: DashMap<PathBuf, AssetKeySourceId>, // maps source paths (relative to sources root) to their source ID. Only tracks sources which have attempted to be built this run
}
impl AssetsBuilder
{
    #[must_use]
    pub fn new(config: AssetsBuilderConfig) -> Self
    {
        // print errors?
        let _ = std::fs::create_dir_all(&config.assets_root);
        let _ = std::fs::create_dir_all(&config.sources_root);

        Self
        {
            config,
            sources: DashMap::new(),
        }
    }

    #[inline] #[must_use]
    pub fn builders_version_hash(&self) -> u64 { self.config.builders_version_hash }

    // Query a source file to get its source ID, or none if the source is not known/does not exist
    pub fn query_source(&self, source_path: impl AsRef<Path>) -> Result<SourceMetadata, MetaFileError>
    {
        // TODO: validate that
        let canonical_path = self.canonicalize_source_path(&source_path).map_err(MetaFileError::FileReadError)?;
        if !std::fs::metadata(&canonical_path).map_err(MetaFileError::FileReadError)?.is_file()
        {
            return Err(MetaFileError::NotAFile);
        }

        // todo: this should probably get its own error
        let source_meta_path = canonical_path.with_extension(AssetsBuilderConfig::SOURCE_META_FILE_EXTENSION.as_ref());
        let mut fin = File::open(&source_meta_path).map_err(MetaFileError::FileReadError)?;
        SourceMetadata::load(&mut fin)
    }

    // Check to see if an asset corresponds to a real asset
    pub fn query_asset(&self, asset_key: AssetKey) -> Result<AssetMetadata, MetaFileError> // TODO: specific error?
    {
        let asset_path = self.config.assets_root.join(&format!("{asset_key:x}.{}", AssetFileType::Asset.file_extension()));
        if !std::fs::metadata(&asset_path).map_err(MetaFileError::FileReadError)?.is_file()
        {
            return Err(MetaFileError::NotAFile);
        }

        // todo: this should probably get its own error
        let asset_meta_path = self.config.assets_root.join(&format!("{asset_key:x}.{}", AssetFileType::MetaData.file_extension()));
        let mut fin = File::open(asset_meta_path).map_err(MetaFileError::FileReadError)?;
        AssetMetadata::load(&mut fin)
    }

    #[inline] #[must_use]
    pub fn scan_sources(&self) -> ScanSources
    {
        let walker = WalkDir::new(&self.config.sources_root);
        ScanSources { walk_dir: walker.into_iter() }
    }

    #[inline] #[must_use]
    pub fn scan_assets(&self) -> ScanAssets
    {
        let walker = WalkDir::new(&self.config.assets_root);
        ScanAssets { walk_dir: walker.into_iter() }
    }

    fn canonicalize_source_path(&self, path: impl AsRef<Path>) -> io::Result<PathBuf>
    {
        (if path.as_ref().is_relative()
        {
            self.config.sources_root.join(path.as_ref())
        }
        else
        {
            path.as_ref().into()
        }).canonicalize()
    }

    pub fn reset_import(&self, source_path: impl AsRef<Path>) -> Result<(), BuildError> // unique error?
    {
        let canonical_path = self.canonicalize_source_path(&source_path)
            .map_err(|e| BuildError::SourceIOError { source_path: source_path.as_ref().to_path_buf(), error: e })?;
        // TODO: remove path from sources list?
        let file_ext = canonical_path.extension().unwrap_or(OsStr::new("")).to_string_lossy();
        let source_meta_file_path = canonical_path.with_extension(
            format!("{}.{}", file_ext.as_ref(), AssetsBuilderConfig::SOURCE_META_FILE_EXTENSION));

        let b_index = self.config.file_ext_to_builder.get(&UniCase::from(file_ext.as_ref()))
            .ok_or(BuildError::NoBuilderForSource { extension: file_ext.to_string() })?;
        let builder = self.config.asset_builders.get(*b_index)
            .expect("Had builder ID but no matching builder!");
        let source_meta= match File::open(&source_meta_file_path)
        {
            Ok(mut fin) =>
            {
                // overwrite on error? will generate new ID
                let meta = SourceMetadata::load(&mut fin)
                    .map_err(|e| BuildError::SourceMetaError { source_meta_path: source_meta_file_path.clone(), error: e })?;
                SourceMetadata
                {
                    source_id: meta.source_id,
                    version_hash: builder.version_hash,
                    build_config: builder.builder.default_config(),
                    notes: None,
                }
            },
            Err(err) if err.kind() == ErrorKind::NotFound =>
            {
                let source_id = AssetKeySourceId::generate();
                SourceMetadata
                {
                    source_id,
                    version_hash: builder.version_hash,
                    build_config: builder.builder.default_config(),
                    notes: None,
                }
            },
            Err(err) =>
            {
                log::error!("Failed to open source asset meta-file for reading: {err}");
                return Err(BuildError::SourceMetaError
                {
                    source_meta_path: source_meta_file_path.clone(),
                    error: MetaFileError::FileReadError(err),
                });
            }
        };

        let mut meta_writer = File::create(&source_meta_file_path)
            .map_err(|err| BuildError::SourceMetaError
            {
                source_meta_path: source_meta_file_path.clone(),
                error: MetaFileError::FileWriteError(err),
            })?;
        source_meta.save(true, &mut meta_writer)
            .map_err(|err| BuildError::SourceMetaError
            {
                source_meta_path: source_meta_file_path.clone(),
                error: err,
            })
    }

    // transform a source file into one or more built asset, returns the built count
    pub fn build_source(&self, source_path: impl AsRef<Path>, build_rule: BuildRule) -> Result<BuildResults, BuildError>
    {
        // TODO: this should enqueue a build rather than build immediate
        let source_path = source_path.as_ref();

        let canonical_path = self.canonicalize_source_path(source_path)
            .map_err(|e| BuildError::SourceIOError { source_path: source_path.to_path_buf(), error: e })?;
        let rel_path = canonical_path.strip_prefix(&self.config.sources_root)
            .map_err(|e| BuildError::InvalidSourcePath { source_path: canonical_path.clone() })?;

        let file_ext = rel_path.extension().unwrap_or(OsStr::new("")).to_string_lossy();

        let b_index = self.config.file_ext_to_builder.get(&UniCase::from(file_ext.as_ref()))
            .ok_or(BuildError::NoBuilderForSource { extension: file_ext.to_string() })?;
        let builder = self.config.asset_builders.get(*b_index)
            .expect("Had builder ID but no matching builder!");

        let source_meta_file_path = canonical_path.with_extension(
            format!("{}.{}", file_ext.as_ref(), AssetsBuilderConfig::SOURCE_META_FILE_EXTENSION));

        let build_time = chrono::Utc::now();

        let (source_meta, meta_modtime) = match File::open(&source_meta_file_path)
        {
            Ok(mut fin) =>
            {
                let meta_modtime = fin.metadata()
                    .map_err(|err| BuildError::SourceMetaError
                    {
                        source_meta_path: source_meta_file_path.clone(),
                        error: MetaFileError::FileReadError(err)
                    })?
                    .modified()
                    .map_err(|err| BuildError::SourceMetaError
                    {
                        source_meta_path: source_meta_file_path.clone(),
                        error: MetaFileError::FileReadError(err)
                    })?;

                let meta = SourceMetadata::load(&mut fin)
                    .map_err(|e| BuildError::SourceMetaError
                    {
                        source_meta_path: source_meta_file_path.clone(),
                        error: e
                    })?;

                (meta, meta_modtime)
            },
            Err(err) if err.kind() == ErrorKind::NotFound =>
            {
                // TODO: assert that thread_rng impls CryptoRng
                // loop while base ID is zero? -- ditto for in reset_import
                let source_id = AssetKeySourceId::generate();

                let new_meta = SourceMetadata
                {
                    source_id,
                    version_hash: builder.version_hash,
                    build_config: builder.builder.default_config(),
                    notes: None,
                };

                {
                    let mut meta_writer = File::create(&source_meta_file_path)
                        .map_err(|err| BuildError::SourceMetaError
                        {
                            source_meta_path: source_meta_file_path.clone(),
                            error: MetaFileError::FileWriteError(err)
                        })?;
                    new_meta.save(true, &mut meta_writer)
                        .map_err(|e| BuildError::SourceMetaError
                        {
                            source_meta_path: source_meta_file_path.clone(),
                            error: MetaFileError::NotAFile,
                        })?;
                }

                debug_assert!(!self.sources.contains_key(rel_path));

                log::info!("{:?} is a new asset, assigned source ID: {source_id:?}", source_path);

                (new_meta, SystemTime::UNIX_EPOCH)
            },
            Err(err) =>
            {
                log::error!("Failed to open source asset meta-file for reading: {err}");
                return Err(BuildError::SourceMetaError
                {
                    source_meta_path: source_meta_file_path.clone(),
                    error: MetaFileError::FileReadError(err),
                });
            }
        };

        // check if this asset has already been built
        let known_entry = self.sources.entry(rel_path.to_path_buf());
        if let dashmap::Entry::Occupied(existing_entry) = known_entry
        {
            if *existing_entry.get() != source_meta.source_id
            {
                log::error!("Source path ID collision: {:?} => {:?} (existing: {:?})",
                    source_path,
                    source_meta.source_id,
                    *existing_entry.get());
            }
            return Ok(BuildResults::default());
        }
        else
        {
            known_entry.insert(source_meta.source_id);
        }

        let mut source_read =
        {
            let fin = File::open(&canonical_path).map_err(|e| BuildError::SourceIOError
            {
                source_path: canonical_path.clone(),
                error: e,
            })?;

            let src_modtime = fin.metadata()
                .map_err(|e| BuildError::SourceIOError
                {
                    source_path: canonical_path.clone(),
                    error: e,
                })?
                .modified()
                .map_err(|e| BuildError::SourceIOError
                {
                    source_path: canonical_path.clone(),
                    error: e,
                })?;
            // note: ideally this checks for matching outputs but 1-many makes that hard
            // TODO: this should actually get the min(built derived asset file times) and diff both src and meta time against it
            if let BuildRule::OnlyIfChanged = build_rule &&
                source_meta.version_hash == builder.version_hash &&
                src_modtime <= meta_modtime
            {
                log::debug!("Skipped (up-to-date) {:?} ({:?})", source_path, source_meta.source_id);
                return Ok(BuildResults::default());
            }

            InlineWriteHash::<MetroHash64, _>::new(Box::new(fin)) // note: seek() makes this hash a bit nondeterministic, but it should be stable as long as the builder/file hasn't changed
        };

        let mut input = SourceInput
        {
            source_path: &canonical_path,
            file_extension: UniCase::from(file_ext),
            source_id: source_meta.source_id,
            input: &mut source_read,
        };

        let mut outputs = BuildOutputs
        {
            assets_builder: &self,
            build_rule,
            source_id: source_meta.source_id,
            timestamp: build_time.clone(),
            rel_source_path: rel_path,
            abs_output_dir: self.config.assets_root.as_path(),
            version_hash: builder.version_hash,
            derived_ids: HashMap::new(),
            results: HashSet::new(),
        };

        let build = builder.builder.build_assets(source_meta.build_config, &mut input, &mut outputs);
        match build
        {
            Ok(_) =>
            {
                // todo: hash can be used for versioning/uniquifying built asssets
                let _input_hash = source_read.finish();

                // bump the modtime for up-to-date tracking
                match File::options().write(true).open(&source_meta_file_path)
                {
                    Ok(fin) =>
                    {
                        if let Err(_) = fin.set_modified(build_time.into())
                        {
                            log::error!("Failed to update {source_meta_file_path:?} write-time");
                        };
                    }
                    Err(_) => log::error!("Failed to open {source_meta_file_path:?} to update write-time"),
                }

                Ok(outputs.results)
            },
            Err(err) => Err(BuildError::BuilderError(err)),
        }
    }

    // Build all (known) sources. Files without an accompanying .sork are skipped
    pub fn build_all(&self, build_rule: BuildRule) -> Result<(), ()> // TODO
    {
        for source in self.scan_sources()
        {
            let Ok((source, _)) = source else { continue };
            let result = self.build_source(&source, build_rule); // TODO
            println!("{source:?} => {:#?}", result);
        }

        Ok(())
    }

    pub fn build_type(&self, asset_type: AssetTypeId, build_rule: BuildRule) -> Result<(), ()> // TODO
    {
        for source in self.scan_sources()
        {
            let Ok((source, _)) = source else { continue };
            let result = self.build_source(&source, build_rule); // TODO
            println!("{source:?} => {:#?}", result);
        }

        Ok(())
    }
    // rebuild_asset(ext, base_id, file_bytes() ?
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum BuildError
{
    InvalidSourcePath { source_path: PathBuf }, // lies outside the sources root
    InvalidSyntheticAssetKey, // asset key was not synthetic
    NoBuilderForSource { extension: String },
    // TODO: add more variants here
    // TODO: add more variants here
    SourceMetaError { source_meta_path: PathBuf, error: MetaFileError },
    AssetMetaError { asset_meta_path: PathBuf, error: MetaFileError },
    UnknownDependency { dependent_asset_key: AssetKey },
    DependencyReadError { dependent_asset_key: AssetKey, error: io::Error },
    DependencyParseError { dependent_asset_key: AssetKey, error: bitcode::Error },
    SourceIOError{ source_path: PathBuf, error: io::Error },
    TooManyDerivedIDs,
    BuilderError(Box<dyn Error>),
    OutputMetaError { output_meta_path: PathBuf, error: MetaFileError },
    OutputError { output_path: PathBuf, error: io::Error },
    OutputDebugError { output_debug_path: PathBuf, error: io::Error },
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

struct BuildOutputInner<'output>
{
    outputs: &'output BuildOutputs<'output>,
    asset_key: AssetKey,
    dependencies: Vec<AssetKey>,
    writer: File,
    meta_writer: File,
    debug_data_file_path: PathBuf,
    name: Option<String>,
}

#[must_use]
pub struct PrimaryOutput<'output, A: Asset>(BuildOutputInner<'output>, PhantomData<A>);
impl<'output, A: Asset> PrimaryOutput<'output, A>
{
    // TODO: it would be great to automate collecting this (macro magic?)
    pub fn add_dependencies(&mut self, dependencies: &[AssetKey]) -> Result<&mut Self, BuildError>
    {
        let builder = self.0.outputs.assets_builder;
        for dependency in dependencies
        {
            match builder.query_asset(*dependency)
            {
                Ok(asset_meta) =>
                {
                    builder.build_source(&asset_meta.source_path, self.0.outputs.build_rule)?;
                }
                Err(e) =>
                {
                    log::error!("Failed to query asset dependency {dependency:?} when building {:?}: {e:?}", self.0.asset_key);
                    return Err(BuildError::UnknownDependency
                    {
                        dependent_asset_key: *dependency,
                    });
                }
            };
        }

        self.0.dependencies.extend_from_slice(dependencies);

        Ok(self)
    }

    pub fn add_and_read_dependency<D: Asset>(&mut self, dependency: AssetKey) -> Result<D::StructuredData, BuildError>
    {
        assert_eq!(dependency.asset_type(), D::asset_type());

        let builder = self.0.outputs.assets_builder;
        let asset_meta = match builder.query_asset(dependency)
        {
            Ok(asset_meta) => asset_meta,
            Err(e) =>
            {
                log::error!("Failed to query asset dependency {dependency:?} when building {:?}: {e:?}", self.0.asset_key);
                return Err(BuildError::UnknownDependency
                {
                    dependent_asset_key: dependency,
                });
            }
        };

        // assert key exists in build results?
        builder.build_source(&asset_meta.source_path, self.0.outputs.build_rule)?;

        let asset_path = builder.config.assets_root.join(&format!("{dependency:x}.{}", AssetFileType::Asset.file_extension()));
        let mut asset_payload = Vec::new();
        let fin = File::open(asset_path).map_err(|e| BuildError::DependencyReadError
            {
                dependent_asset_key: dependency,
                error: e,
            })?
            .read_to_end(&mut asset_payload).map_err(|e| BuildError::DependencyReadError
            {
                dependent_asset_key: dependency,
                error: e,
            })?;


        let (structured_data_size, structured_data_start) = varint::decode(&mut asset_payload);
        let structured_data_end = structured_data_start + structured_data_size as usize;
        if structured_data_start == 0 ||
            asset_payload.len() < structured_data_end
        {
            return Err(BuildError::DependencyReadError
            {
                dependent_asset_key: dependency,
                error: io::Error::from(io::ErrorKind::UnexpectedEof),
            })
        }

        let bytes = &asset_payload[structured_data_start..structured_data_end];
        bitcode::decode::<D::StructuredData>(bytes).map_err(|e| BuildError::DependencyParseError
        {
            dependent_asset_key: dependency,
            error: e,
        })
    }

    pub fn write_structured(mut self, value: &A::StructuredData) -> io::Result<OpaqueOutput<'output, A>>
    {
        let val = bitcode::encode(value);
        varint::encode_into(val.len() as u64, &mut self.0.writer)?;
        self.0.writer.write_all(val.as_slice())?;
        self.0.writer.flush()?;
        Ok(OpaqueOutput(self.0, PhantomData))
    }
}
#[must_use]
pub struct OpaqueOutput<'output, A: Asset>(BuildOutputInner<'output>, PhantomData<A>);
impl<'output, A: Asset> OpaqueOutput<'output, A>
{
    pub fn skip_opaque(mut self) -> DebugOutput<'output, A>
    {
        DebugOutput(self.0, PhantomData)
    }

    // todo: better design for streaming writes?
    pub fn write_into_opaque(mut self, writer_fn: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<DebugOutput<'output, A>>
    {
        writer_fn(&mut self.0.writer)?;
        self.0.writer.flush()?;
        Ok(DebugOutput(self.0, PhantomData))
    }

    pub fn write_opaque<T>(mut self, opaque: T) -> io::Result<DebugOutput<'output, A>>
    {
        self.write_opaque_bytes(unsafe { val_as_u8_slice(&opaque) })
    }

    pub fn write_opaque_bytes(mut self, opaque_bytes: &[u8]) -> io::Result<DebugOutput<'output, A>>
    {
        self.0.writer.write_all(opaque_bytes)?;
        self.0.writer.flush()?;
        Ok(DebugOutput(self.0, PhantomData))
    }
}
#[must_use]
pub struct DebugOutput<'output, A: Asset>(BuildOutputInner<'output>, PhantomData<A>);
impl<'output, A: Asset> DebugOutput<'output, A>
{
    pub fn skip_debug(self) -> FinalizedOutput<'output> { FinalizedOutput(self.0) }
    pub fn write_debug(mut self, value: &A::DebugData) -> io::Result<FinalizedOutput<'output>>
    {
        let mut debug_writer = File::create(&self.0.debug_data_file_path)?;
        let val = bitcode::encode(value);
        debug_writer.write_all(val.as_slice())?;
        Ok(FinalizedOutput(self.0))
    }
}
#[must_use]
pub struct FinalizedOutput<'output>(BuildOutputInner<'output>);
impl<'output> FinalizedOutput<'output>
{
    pub fn finish(mut self, name: Option<String>) -> Result<AssetKey, BuildError>
    {
        self.0.name = name;
        self.0.writer.flush().map_err(|e| BuildError::OutputError
        {
            output_path: self.0.asset_key.as_file_name(AssetFileType::Asset).into(), // todo: full path?
            error: e,
        })?;

        self.0.dependencies.dedup();

        // TODO: this can be pulled back into BuildOutputs

        // write metadata
        let asset_meta = AssetMetadata
        {
            key: self.0.asset_key,
            name: self.0.name,
            source_path: self.0.outputs.rel_source_path.to_path_buf(),
            build_timestamp: chrono::Utc::now(),
            version_hash: self.0.outputs.version_hash,
            dependencies: self.0.dependencies.into_boxed_slice(),
        };
        // TODO: read old file and compare asset key

        asset_meta.save(false, &mut self.0.meta_writer).map_err(|e| BuildError::OutputMetaError
        {
            output_meta_path: self.0.asset_key.as_file_name(AssetFileType::MetaData).into(), // todo: full path?
            error: e,
        })?;

        Ok(self.0.asset_key)
    }
}

pub struct BuildOutputs<'build>
{
    assets_builder: &'build AssetsBuilder,
    build_rule: BuildRule,
    source_id: AssetKeySourceId,
    timestamp: chrono::DateTime<chrono::Utc>,

    rel_source_path: &'build Path,
    abs_output_dir: &'build Path,

    version_hash: VersionHash,
    derived_ids: HashMap<AssetTypeId, AssetKeyDerivedId>,

    results: BuildResults,
}
impl<'build> BuildOutputs<'build>
{
    // TODO: outputs should be atomic (all or none)

    #[inline] #[must_use]
    pub fn source_path(&self) -> &Path { self.rel_source_path }

    // Produce an output from this build. Assets of the same type have sequential derived IDs
    #[inline]
    pub fn add_output<A: Asset>(&mut self)
        -> Result<PrimaryOutput<'_, A>, BuildError>
    {
        let derived_id: AssetKeyDerivedId =
        {
            let entry = self.derived_ids
                .entry(A::asset_type())
                .or_insert(AssetKeyDerivedId::default());
            entry.next().ok_or(BuildError::TooManyDerivedIDs)?
        };

        let asset_key = AssetKey::unique(A::asset_type(), derived_id, self.source_id);
        self.add_asset(asset_key, BuildRule::ForceBuildAll)
    }

    // Produce an output from ths build that is referenced by a calculable hash. By default, will only return an output if the hash doesn't already exist
    #[inline]
    pub fn add_synthetic<A: Asset>(&mut self, asset_hash: AssetKeySynthHash)
                                   -> Result<PrimaryOutput<'_, A>, BuildError>
    {
        let asset_key = AssetKey::synthetic(A::asset_type(), asset_hash);
        self.add_asset(asset_key, self.build_rule)
    }

    // TODO: this should probably just care about source changing and not outputs
    // build an asset (if rules allow) and add an output to the asset build
    fn add_asset<A: Asset>(&mut self, asset_key: AssetKey, _build_rule: BuildRule)
                           -> Result<PrimaryOutput<'_, A>, BuildError>
    {
        let output_path = self.abs_output_dir.join(asset_key.as_file_name(AssetFileType::Asset));
        let output_meta_path = self.abs_output_dir.join(asset_key.as_file_name(AssetFileType::MetaData));

        // TODO ?
        // let _should_build = match build_rule
        // {
        //     BuildRule::OnlyIfChanged =>
        //     {
        //         !output_path.exists() ||
        //         {
        //             match File::open(&output_meta_path)
        //             {
        //                 Ok(mut fin) =>
        //                 {
        //                     let mut bytes = Vec::new();
        //                     fin.read_to_end(&mut bytes)
        //                         .map_err(|err| BuildError::AssetMetaError
        //                         {
        //                             asset_meta_path: output_meta_path.clone(),
        //                             error: MetaFileError::FileReadError(err),
        //                         })?;
        //                     let meta = AssetMetadata::load(&mut Cursor::new(bytes))
        //                         .map_err(|e| BuildError::AssetMetaError
        //                         {
        //                             asset_meta_path: output_meta_path.clone(),
        //                             error: e,
        //                         })?;
        //
        //                     // log for each reason?
        //                     meta.version_hash != self.version_hash
        //                     // check timestamps?
        //                 }
        //                 Err(e) if e.kind() == io::ErrorKind::NotFound => true,
        //                 // TODO: just overwrite?
        //                 Err(e) => return Err(BuildError::AssetMetaError
        //                 {
        //                     asset_meta_path: output_meta_path.clone(),
        //                     error: MetaFileError::FileReadError(e),
        //                 }),
        //             }
        //         }
        //     },
        //     BuildRule::ForceBuildAll => true,
        // };

        assert!(!self.results.contains(&asset_key));
        // TODO
        // if should_build
        {
            let output_writer = File::create(&output_path)
                .map_err(|e| BuildError::OutputError
                {
                    output_path: output_path.clone(),
                    error: e,
                })?;
            let output_meta_writer = File::create(&output_meta_path)
                .map_err(|e| BuildError::OutputMetaError
                {
                    output_meta_path: output_meta_path.clone(),
                    error: MetaFileError::FileWriteError(e),
                })?;
            let output_debug_path = self.abs_output_dir.join(asset_key.as_file_name(AssetFileType::DebugData));

            log::debug!("Building {:#?}", asset_key);

            self.results.insert(asset_key);

            Ok(PrimaryOutput(BuildOutputInner
            {
                outputs: self,
                asset_key,
                dependencies: Vec::new(),
                writer: output_writer,
                meta_writer: output_meta_writer,
                debug_data_file_path: output_debug_path,
                name: None,
            }, PhantomData))
        }
    }
}

pub trait SourceInputRead: Read + Seek { }
impl<T: Read + Seek> SourceInputRead for T { }

pub struct SourceInput<'builder>
{
    source_path: &'builder Path, // The full, absolute path of the source file
    file_extension: UniCase<String>, // does not include .
    source_id: AssetKeySourceId,
    input: &'builder mut dyn SourceInputRead,
}
impl<'builder> SourceInput<'builder>
{
    // Return the full, absolute path of the source file
    #[inline] #[must_use]
    pub fn source_path(&self) -> &Path { self.source_path }
    #[inline] #[must_use]
    pub fn source_path_string(&self) -> String { self.source_path.to_string_lossy().to_string() }
    #[inline] #[must_use]
    pub fn file_extension(&self) -> &UniCase<String> { &self.file_extension }
}
impl<'builder> Read for SourceInput<'builder>
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.input.read(buf) }
}
impl<'builder> Seek for SourceInput<'builder>
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> { self.input.seek(pos) }
}
// todo:
