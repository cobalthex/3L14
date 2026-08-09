use std::error::Error;
use std::io::Read;
use arrayvec::ArrayVec;
use serde::{Deserialize, Serialize};
use asset_3l14::{AssetKey, AssetTypeId};
use graphics_3l14::assets::{Material, MaterialFile};
use graphics_3l14::material_classes::{MaterialClass, MaterialDef, PbrProps};
use nab_3l14::utils::alloc_slice::alloc_u8_slice;
use crate::core::{AssetBuilder, BuildOutputs, SourceInput, VersionBuilder};

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
        let material_def: MaterialDef = toml::from_str(&toml_str)?;

        // todo: shader dependencies
        // todo: texture dependencies

        let material_file = match material_def
        {
            MaterialDef::DebugLines =>
            {
                MaterialFile
                {
                    class: MaterialClass::DebugLines,
                    textures: ArrayVec::default(),
                    props: Box::new([]),
                }
            }
            MaterialDef::PbrOpaque { albedo_tex, props } =>
            {
                outputs.add_dependency(albedo_tex)?;
                let mut textures = ArrayVec::new();
                textures.push(albedo_tex);

                MaterialFile
                {
                    class: MaterialClass::DebugLines,
                    textures,
                    props: alloc_u8_slice(props),
                }
            }
        };
        
        outputs.add_output(AssetTypeId::Material, |output|
        {
           output.serialize(&material_file)?;
            Ok(())
        })?;

        Ok(())
    }
}
