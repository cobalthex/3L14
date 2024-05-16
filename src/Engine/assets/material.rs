use crate::engine::assets::texture::{Texture};
use super::*;

/*

Materials can exist 

 */

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

// impl<'a> DebugGui<'a> for MaterialLifecycler
// {
//     fn name(&self) -> &'a str { "Materials" }
// 
//     fn debug_gui(&self, ui: &mut Ui)
//     {
// 
//     }
// }

// material loads/owns texture and creates view for texture