use std::error::Error;
use std::io::Read;
use serde::{Deserialize, Serialize};
use asset_3l14::AssetTypeId;
use graphics_3l14::assets::{Material, MaterialFile};
use graphics_3l14::material_classes::PbrProps;
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionBuilder};

#[derive(Serialize, Deserialize)]
pub enum MaterialDef
{
    PbrOpaque
    {
        pbr: PbrProps,
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct MaterialBuilderConfig
{
}

pub struct MaterialBuilder;
impl AssetBuilder for MaterialBuilder
{
    type BuildConfig = MaterialBuilderConfig;

    fn supported_input_file_extensions(&self) -> &'static [&'static str]
    {
        &["matl"]
    }

    fn builder_version(&self, vb: &mut VersionBuilder)
    {
        vb.push(b"Material builder - initial");
    }

    fn format_version(&self, vb: &mut VersionBuilder)
    {
        vb.push_prehashed(Material::TYPE_LAYOUT_HASH);
    }

    fn build_assets(&self, config: Self::BuildConfig, input: &mut SourceInput, outputs: &mut BuildOutputs) -> Result<(), Box<dyn Error>>
    {
        let mut toml_str = String::new();
        input.read_to_string(&mut toml_str)?;
        let mtl_def: MaterialDef = toml::from_str(&toml_str)?;

        // todo: shader dependencies
        // todo: texture dependencies

        match mtl_def
        {
            MaterialDef::PbrOpaque { pbr} =>
            {
                
            }
        }
        
        outputs.add_output(AssetTypeId::Material, |output|
        {
           output.serialize(&mtl_def)?;
            Ok(())
        })?;

        Ok(())
    }
}
