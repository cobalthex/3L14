use std::collections::HashMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Read;
use serde::{Deserialize, Serialize};
use containers_3l14::AabbTree;
use math_3l14::AABB;
use nab_3l14::utils::osstr::OsStrUtils;
use source_defs_3l14::{MapDef, MapLayer};
use world_3l14::assets::map::{MapFile, StaticsFile};
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionBuilder};

#[derive(Default, Serialize, Deserialize)]
pub struct MapBuilderConfig
{

}

#[derive(Debug)]
enum MapBuildError
{
    NoLayers,
    TooManyModels,
}
impl std::fmt::Display for MapBuildError
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            MapBuildError::NoLayers => write!(f, "Map definitions must have at least one layer"),
            MapBuildError::TooManyModels => write!(f, "Map contains too many unique models"),
        }
    }
}
impl std::error::Error for MapBuildError {}

pub struct MapBuilder;
impl AssetBuilder for MapBuilder
{
    type BuildConfig = MapBuilderConfig;

    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["mapdef"]
    }

    fn builder_version(&self, vb: &mut VersionBuilder)
    {
        vb.push(b"Map builder - initial");
    }

    fn format_version(&self, vb: &mut VersionBuilder)
    {
        vb.push_prehashed(MapFile::TYPE_LAYOUT_HASH);
    }

    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mut toml_str = String::new();
        let map_def: MapDef =
        {
            input.read_to_string(&mut toml_str)?;
            toml::from_str(toml_str.as_str())?
        };

        // should layers be explicitly referenced?
        let layers =
        {
            // does this break down if building from memory is possible?
            // let map_def_name = input.source_path().file_name().expect("How did the input not have a file name?");
            let map_def_dir = input.source_path().parent().expect("How did the input not exist in a directory?");

            let layer_def_suffix =
            {
                let mut str = OsString::from(".");
                str.push(input.source_path().file_stem().expect("How did the input not have a file stem?"));
                str.push(OsStr::new(".layerdef"));
                str
            };

            let mut layers = Vec::new();
            for layer_file in map_def_dir.read_dir()?
            {
                let layer_path = layer_file?.path();
                if !layer_path.is_file() { continue; }
                let Some(layer_name) = layer_path
                    .file_name()
                    .expect("How did the layer not have a file name?")
                    .strip_suffix(&layer_def_suffix)
                else { continue };

                let layer_name = layer_name.to_string_lossy().to_string();

                toml_str.clear();
                File::open(layer_path)?
                    .read_to_string(&mut toml_str)?;
                layers.push((layer_name, toml::from_str::<MapLayer>(toml_str.as_str())?));
            }
            layers
        };

        let mut model_palette = HashMap::new();
        let mut insert_model = |model_key: asset_3l14::AssetKey|
        {
            let new_index = model_palette.len();
            let entry = model_palette.entry(model_key)
                .or_insert_with(|| (model_palette.len() as u32, true));
            if new_slot
            {
                outputs.add_dependency(model_key)?;

            }
            if index > u32::MAX as usize
            {
                return Err(MapBuildError::TooManyModels);
            }
            Ok(index as u32)
        };

        let mut statics_geo = Vec::new();
        let mut statics_aabb = AabbTree::new();

        for (layer_name, layer) in &layers
        {
            for model in layer.models.iter()
            {
                let palette_index = insert_model(model.model)?;
                let geo_index = statics_geo.len() as u32;
                statics_geo.push(palette_index);
                statics_aabb.insert(TODO, geo_index);
            }
        }

        let map_file = MapFile
        {
            model_palette: model_palette.drain().map(|asset_key| asset_key).collect(),
            statics: StaticsFile
            {
                hierarchy: statics_aabb,
                geo: statics_geo.into_boxed_slice(),
                lights: Box::new([]),
            },
        };

        todo!()
    }
}
