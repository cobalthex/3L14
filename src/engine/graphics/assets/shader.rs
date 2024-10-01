use std::borrow::Cow;
use std::error::Error;
use std::io::Read;
use std::sync::Arc;
use wgpu::ShaderModule;
use wgpu::{ShaderModuleDescriptor, ShaderSource};
use crate::engine::assets::{Asset, AssetLifecycler, AssetLoadError, AssetLoadRequest, AssetPayload, AssetTypeId};
use crate::engine::graphics::Renderer;

pub struct Shader
{
    pub module: ShaderModule,
    // entry_point: String,
    // stage: ShaderStage,
}
impl Shader
{
}
impl Asset for Shader
{
    fn asset_type() -> AssetTypeId { AssetTypeId::Shader }
}

pub struct ShaderLifecycler
{
    renderer: Arc<Renderer>,
}
impl ShaderLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }
}
impl AssetLifecycler for ShaderLifecycler
{
    type Asset = Shader;
    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let mut source_text = String::new();
        request.input.read_to_string(&mut source_text)?;

        let module = self.renderer.device().create_shader_module(ShaderModuleDescriptor
        {
            label: Some(&format!("{:?}", request.asset_key)),
            source: ShaderSource::Wgsl(Cow::from(source_text)),
        });

        Ok(Shader
        {
            module,
        })
    }
}
// impl<'a> DebugGui<'a> for ShaderLifecycler
// {
//     fn name(&self) -> &'a str { "Shaders" }
//
//     fn debug_gui(&self, ui: &mut Ui)
//     {
//
//     }
// }