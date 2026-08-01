use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Read;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use map_design_3l14::{MapDef, MapLayer};
use nab_3l14::utils::osstr::OsStrUtils;
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionBuilder};
use world_3l14::MapFile;

#[derive(Default, Serialize, Deserialize)]
pub struct MapBuilderConfig
{

}

#[derive(Debug)]
enum MapBuildError
{
    NoLayers,
}
impl std::fmt::Display for MapBuildError
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            MapBuildError::NoLayers => write!(f, "Map definitions must have at least one layer"),
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
                println!(">> {:?}", layer_path);
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
        println!("map: {}", map_def.name);
        for (layer_name, _) in &layers
        {
            println!("layer: {}", layer_name);
        }
        todo!()
    }
}
