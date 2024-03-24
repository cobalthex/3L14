use super::{Asset, AssetHandle, AssetLifecycler, AssetLoadRequest};

pub struct Material
{

}
impl Material
{

}
impl Asset for Material
{
}

pub struct MaterialLifecycler
{

}
impl MaterialLifecycler
{

}
impl AssetLifecycler for MaterialLifecycler
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