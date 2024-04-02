use arc_swap::ArcSwap;
use super::*;

pub struct Material
{

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
    fn get_or_create(&self, request: AssetLoadRequest) -> ArcSwap<AssetPayload<Material>>
    {
        todo!()
    }
    
    fn stats(&self) -> AssetLifecyclerStats
    {
        AssetLifecyclerStats
        {
            active_assets: 0,
        }
    }
}