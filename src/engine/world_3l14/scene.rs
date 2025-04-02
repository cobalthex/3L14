use glam::Vec3;
use asset_3l14::AssetHandle;
use graphics_3l14::assets::Model;
use math_3l14::{Radians, AABB};

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

#[derive(Default)] // temp?
struct Statics
{
    geo: Vec<AssetHandle<Model>>,
    lights: Vec<Light>,
}

pub struct Scene
{
    statics: Statics,
    
}
impl Scene
{
    pub fn new() -> Self
    {
        Self
        {
            statics: Statics::default(),
        }
    }
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