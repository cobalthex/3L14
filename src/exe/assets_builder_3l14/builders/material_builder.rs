use std::error::Error;
use std::io::{Read, Write};
use arrayvec::ArrayVec;
use serde::{Deserialize, Serialize};
use asset_3l14::{AssetKey, AssetTypeId};
use graphics_3l14::assets::{shader_key, Material, MaterialFile, EngineRenderPass};
use graphics_3l14::material_classes::{MaterialClass, MaterialDef, PbrProps};
use nab_3l14::utils::alloc_slice::alloc_u8_slice;
use nab_3l14::utils::{val_as_u8_slice, AsU8Slice};
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

        let material_file = match material_def
        {
            MaterialDef::DebugLines =>
            {
                MaterialFile
                {
                    class: MaterialClass::DebugLines,
                    textures: ArrayVec::default(),
                }
            }
            MaterialDef::PbrOpaque { albedo_tex, .. } =>
            {
                let mut textures = ArrayVec::new();
                textures.push(albedo_tex);

                MaterialFile
                {
                    class: MaterialClass::PbrOpaque,
                    textures,
                }
            }
        };

        // todo: shader dependencies

        let mut out = outputs.add_output::<Material>()?;
        out.add_dependencies(&material_file.textures)?;

        let shader_akey = AssetKey::synthetic(
            AssetTypeId::Shader,
            shader_key::pixel(material_file.class, EngineRenderPass::Opaque));
        out.add_dependencies(&[shader_akey])?;

        let mut out = out.write_structured(&material_file)?;
        let out = match material_def
        {
            MaterialDef::DebugLines => out.skip_opaque(),
            MaterialDef::PbrOpaque { props, .. } =>
            {
                out.write_opaque_bytes(unsafe { val_as_u8_slice(&props) })?
            }
        };

        out.skip_debug()
            .finish(None)?; // TODO: name

        Ok(())
    }
}
