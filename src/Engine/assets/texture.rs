use arc_swap::ArcSwap;
use super::*;

pub struct Texture
{

}
impl Texture
{

}
impl Asset for Texture
{
}

#[derive(Default)]
pub struct TextureLifecycler
{
    
}
impl TextureLifecycler
{

}
impl AssetLifecycler<Texture> for TextureLifecycler
{
    fn get_or_create(&self, request: AssetLoadRequest) -> ArcSwap<AssetPayload<Texture>>
    {
        todo!("This is a test")
    }

    fn stats(&self) -> AssetLifecyclerStats
    {
        AssetLifecyclerStats
        {
            active_assets: 0,
        }
    }
}