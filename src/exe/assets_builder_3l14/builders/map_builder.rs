use std::error::Error;
use std::io::Read;
use serde::{Deserialize, Serialize};
use map_design_3l14::{MapDef, MapLayer};
use nab_3l14::utils::osstr::OsStrUtils;
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionBuilder};
use world_3l14::MapFile;

#[derive(Default, Serialize, Deserialize)]
pub struct MapBuilderConfig
{

}

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
            let map_def_name = input.source_path().file_name().expect("How did the input not have a file name?");
            let map_def_dir = input.source_path().parent().expect("How did the input not exist in a directory?");

            let mut layers = Vec::new();
            for layer_file in map_def_dir.read_dir()?
            {
                let layer_path = layer_file?.path();
                if !layer_path.is_file() { continue; }
                let layer_file_name = layer_path.file_name()
                    .expect("How did the layer not have a file name?")
                    .ends_with(map_def_name);

                toml_str.clear();
                
                layers.push(toml::from_str::<MapLayer>(toml_str.as_str())?);
            }
            layers
        };

        todo!()
    }
}
