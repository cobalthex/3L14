use std::borrow::Cow;
use std::io::Read;
use wgpu::ShaderModule;
use wgpu::{ShaderModuleDescriptor, ShaderSource};
use crate::engine::assets::{Asset, AssetLifecycler, AssetLoadError, AssetLoadRequest};
use crate::engine::graphics::Renderer;

struct Shader
{
    module: ShaderModule,
    // entry_point: String,
    // stage: ShaderStage,
}
impl Shader
{
}
impl Asset for Shader { }

struct ShaderLifecycler<'r>
{
    renderer: &'r Renderer<'r>,
}
impl<'r> AssetLifecycler<Shader> for ShaderLifecycler<'r>
{
    fn create_or_update(&self, mut request: AssetLoadRequest<Shader>)
    {
        let mut source_text = String::new();

        match request.input.read_to_string(&mut source_text)
        {
            Ok(_) => {}
            Err(err) =>
            {
                eprintln!("Failed to load shader: {err}");
                request.error(AssetLoadError::ParseError);
                return;
            }
        }

        let module = self.renderer.device().create_shader_module(ShaderModuleDescriptor
        {
            label: None, // TODO
            source: ShaderSource::Wgsl(Cow::from(source_text)),
        });

        request.finish(Shader
        {
            module,
        });
    }
}