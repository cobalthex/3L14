use crate::engine::assets::*;
use crate::engine::graphics::assets::texture::Texture;

pub struct Material
{
    pub albedo_map: AssetHandle<Texture>,
}
impl Material
{

}
impl Asset for Material
{
}

#[derive(Default)]
pub struct MaterialLifecycler
{

}
impl MaterialLifecycler
{

}
impl AssetLifecycler<Material> for MaterialLifecycler
{
    fn create_or_update(&self, request: AssetLoadRequest<Material>)
    {
        todo!()
    }
}