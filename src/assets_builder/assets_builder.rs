use crate::asset_builder::{AssetBuilder, BuildError, BuildOutputWrite, BuildOutputs, SourceInput, SourceMetadata};
use game_3l14::engine::assets::{AssetKey, AssetKeyBaseId, AssetKeyDerivedId, AssetMetadata, AssetTypeId, BuilderHash};
use game_3l14::engine::ShortTypeName;
use metrohash::MetroHash64;
use rand::RngCore;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::hash::Hasher;
use std::io::{ErrorKind, Read, Seek};
use std::path::{Path, PathBuf};
use unicase::UniCase;

struct AssetBuilderEntry
{
    name: &'static str,
    builder_hash: BuilderHash,
    format_hash: BuilderHash,
    builder: Box<dyn AssetBuilder>,
}

pub struct AssetsBuilderConfig
{
    pub source_files_root: PathBuf,
    pub built_files_root: PathBuf,

    builders_version_hash: u64,
    asset_builders: Vec<AssetBuilderEntry>,
    file_ext_to_builder: HashMap<UniCase<&'static str>, usize>,
}
impl AssetsBuilderConfig
{
    pub const SOURCE_FILE_META_EXTENSION: UniCase<&'static str> = UniCase::unicode("sork");

    pub fn builders_version_hash(&self) -> u64 { self.builders_version_hash }

    fn hash_bstrings(seed: u64, bstrings: &[&[u8]]) -> u64
    {
        let mut hasher = MetroHash64::with_seed(seed);
        bstrings.iter().for_each(|s| { hasher.write(s); });
        hasher.finish()
    }

    pub fn new<P: AsRef<Path>>(source_files_root: P, built_files_root: P) -> Self
    {
        Self
        {
            source_files_root: PathBuf::from(source_files_root.as_ref()),
            built_files_root: PathBuf::from(built_files_root.as_ref()),
            builders_version_hash: Self::hash_bstrings(0,&[
                b"Initial"
            ]),
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
            builder_hash: Self::hash_bstrings(self.builders_version_hash, builder.builder_version()),
            format_hash: Self::hash_bstrings(0, builder.format_version()),
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

    pub fn build_assets<P: AsRef<Path> + Debug>(&self, source_path: P) -> Result<(), BuildError>
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
        }.canonicalize().map_err(|e| BuildError::SourceIOError(e))?;

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

                let meta_write = std::fs::File::create(&source_meta_file_path).map_err(|e| BuildError::SourceMetaIOError(e))?;
                ron::ser::to_writer_pretty(meta_write, &new_meta, ron::ser::PrettyConfig::new().compact_arrays(true))
                    .map_err(|e| BuildError::SourceMetaSerializeError(e))?;

                new_meta
            },
            Err(err) =>
            {
                println!("Failed to open source asset meta-file for reading: {err}");
                return Err(BuildError::SourceMetaIOError(err));
            }
        };

        let source_read = std::fs::File::open(&canonical_path).map_err(|e| BuildError::SourceIOError(e))?;

        let input = SourceInput
        {
            file_extension: UniCase::from(file_ext),
            base_id: source_meta.base_id,
            input: Box::new(source_read),
        };

        let mut output = BuildOutputs
        {
            base_id: source_meta.base_id,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            rel_source_path: rel_path,
            abs_output_dir: self.config.built_files_root.as_path(),
            builder_hash: builder.builder_hash,
            format_hash: builder.format_hash,

            derived_ids: HashMap::new(),
            outputs_count: 0,
        };

        // eprintln!("Successfully built {:?} with {} builder into {} assets",
        //           rel_path, builder.name, output.outputs_count());
        builder.builder.build_assets(input, &mut output)
    }

    // rebuild_asset(ext, base_id, file_bytes() ?
}

pub(super) struct BuildOutputs<'a>
{
    base_id: AssetKeyBaseId,
    timestamp: u64, // unix timestamp in milliseconds

    builder_hash: BuilderHash,
    format_hash: BuilderHash,

    rel_source_path: &'a Path,
    abs_output_dir: &'a Path,

    derived_ids: HashMap<AssetTypeId, AssetKeyDerivedId>,
    outputs_count: usize,
}
impl<'a> BuildOutputs<'a>
{
    pub fn outputs_count(&self) -> usize { self.outputs_count }

    pub fn write_output(&mut self, asset_type: AssetTypeId) -> Result<impl BuildOutputWrite, BuildError>
    {
        self.outputs_count += 1;

        let derived_id: AssetKeyDerivedId =
            {
                let entry = self.derived_ids.entry(asset_type).or_insert(0);
                entry.checked_add(1).ok_or(BuildError::TooManyDerivedIDs)? - 1
            };

        let asset_meta = AssetMetadata
        {
            key: AssetKey::new(asset_type, derived_id, self.base_id),
            build_timestamp: self.timestamp,
            builder_hash: self.builder_hash,
            format_hash: self.format_hash,
            source_path: self.rel_source_path.to_path_buf(),
        };

        // TODO: read old file and compare asset key

        let output_path = self.abs_output_dir.join(asset_meta.key.as_file_name());
        let output_writer = std::fs::File::create(&output_path).map_err(|e| BuildError::OutputIOError(e))?;

        // TODO: write asset metadata first

        postcard::to_io(&asset_meta, &output_writer).map_err(|e| BuildError::OutputSerializeError(e))?;

        Ok(output_writer)
    }
}

pub trait SourceInputRead: Read + Seek { }
impl<T: Read + Seek> SourceInputRead for T { }

pub(super) struct SourceInput
{
    pub file_extension: UniCase<String>, // does not include .
    pub base_id: AssetKeyBaseId,
    pub input: Box<dyn SourceInputRead>,

    // TODO: this needs dependency support
}
