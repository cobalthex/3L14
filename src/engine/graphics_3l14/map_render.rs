use asset_3l14::{Ash, AssetView, Assets};
use world_3l14::assets::map::Map;
use crate::assets::Model;
use crate::view::View;

pub struct MapRenderer
{
    map_handle: AssetView<Map>,

    model_palette: Vec<Ash<Model>>,

}
impl MapRenderer
{
    #[must_use]
    pub fn new(
        assets: &Assets,
        map: AssetView<Map>) -> Self
    {
        let model_palette = map.model_palette.iter()
            .map(|m| assets.load::<Model>(*m))
            .collect();

        Self
        {
            map_handle: map,
            model_palette,
        }
    }

    pub fn render(&self, view: &mut View)
    {

    }
}