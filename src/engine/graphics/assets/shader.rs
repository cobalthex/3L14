use std::borrow::Cow;
use std::io::Read;
use std::sync::Arc;
use wgpu::ShaderModule;
use wgpu::{ShaderModuleDescriptor, ShaderSource};
use crate::engine::assets::{Asset, AssetLifecycler, AssetLoadError, AssetLoadRequest, AssetPayload};
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
impl Asset for Shader { }

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
    fn load(&self, mut request: AssetLoadRequest) -> AssetPayload<Self::Asset>
    {
        let mut source_text = String::new();
        match request.input.read_to_string(&mut source_text)
        {
            Ok(_) => {}
            Err(err) =>
            {
                eprintln!("Failed to load shader: {err}");
                return AssetPayload::Unavailable(AssetLoadError::ParseError(err.kind() as u16));
            }
        }

        let module = self.renderer.device().create_shader_module(ShaderModuleDescriptor
        {
            label: None, // TODO
            source: ShaderSource::Wgsl(Cow::from(source_text)),
        });

        AssetPayload::Available(Shader
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