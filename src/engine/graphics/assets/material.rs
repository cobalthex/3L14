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
impl AssetLifecycler for MaterialLifecycler
{
    type Asset = Material;
    fn load(&self, request: AssetLoadRequest) -> AssetPayload<Self::Asset>
    {
        let tex = request.load_dependency(&"assets/test.png");
        AssetPayload::Available(Material
        {
            albedo_map: tex,
        })
    }
}