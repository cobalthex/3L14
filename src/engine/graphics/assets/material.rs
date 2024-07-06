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
impl<L: AssetLifecyclers> AssetLifecycler<Material, L> for MaterialLifecycler
    where for <'l> L: AssetLifecyclerLookup<Texture> + 'l
{
    fn create_or_update(&self, mut request: AssetLoadRequest<Material, L>)
    {
        let tex = request.load_dependency(&"assets/test.png");
        request.finish(Material
        {
            albedo_map: tex,
        })
    }
}