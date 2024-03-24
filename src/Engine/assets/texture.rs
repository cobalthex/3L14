use super::{Asset, AssetHandle, AssetLifecycler, AssetLoadRequest};

pub struct Texture
{

}
impl Texture
{

}
impl Asset for Texture
{
}

pub struct TextureLifecycler
{

}
impl TextureLifecycler
{

}
impl AssetLifecycler for TextureLifecycler
{
    fn load(&mut self, request: AssetLoadRequest)
    {
        todo!()
    }

    fn unload(&mut self, handle: AssetHandle)
    {
        todo!()
    }
}