use glam::Vec3;
use crate::engine::asset::AssetHandle;
use crate::engine::math::{Radians, AABB};
use crate::engine::graphics::assets::model::*;

pub struct SpatialMapIter<'s, T>
{
    map: &'s SpatialMap<T>,
    overlapping: Vec<usize>,
}

pub struct SpatialMap<T>
{
    buckets: Vec<Vec<T>>, // todo
}
impl<T> SpatialMap<T>
{
    pub fn get(&self, region: AABB) -> SpatialMapIter<'_, T>
    {
        todo!()
    }
}

pub struct Scene
{
    pub static_geo: Vec<AssetHandle<Model>>,
}
impl Scene
{

}

pub enum Light
{
    Point(Vec3),
    Directional(Vec3),
    Spot
    {
        angle: Radians,
        range: f32,
    },
    // rect/disc area lights
}