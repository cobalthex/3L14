use crate::engine::asset::{Asset, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use crate::engine::graphics::Renderer;
use std::error::Error;
use std::io::Read;
use std::sync::Arc;
use wgpu::ShaderModule;
use wgpu::ShaderModuleDescriptor;

pub struct Shader
{
    pub module: ShaderModule,
    // entry_point: String,
    // stage: ShaderStage,
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
        let mut module_bytes = Vec::new();
        request.input.read_to_end(&mut module_bytes)?;

        let module = self.renderer.device().create_shader_module(ShaderModuleDescriptor
        {
            label: Some(&format!("{:?}", request.asset_key)),
            source: wgpu::util::make_spirv(&module_bytes),
        });

        Ok(Shader
        {
            module,
        })
    }
}