use std::cmp::Ordering;
use std::collections::hash_map::Drain;
use std::collections::HashMap;
use triomphe::Arc;
use arrayvec::ArrayVec;
use glam::Mat4;
use crate::assets::{Geometry, Material, Shader, Texture, MAX_MATERIAL_TEXTURE_BINDINGS};
use crate::pipeline_cache::PipelineKey;

pub struct Draw
{
    pub transform: Mat4,
    pub depth: f32,
    pub mesh_index: u32,
    pub transform_uniform_id: u32,
    pub poses_uniform_id: Option<u32>, // separate draw call?
    pub pipeline_hash: PipelineKey,
    pub geometry: Arc<Geometry>,
    pub material: Option<(
        Arc<Material>,
        ArrayVec<Arc<Texture>, MAX_MATERIAL_TEXTURE_BINDINGS>
    )>, 
}
// vertex textures?

pub enum SortDirection
{
    Unsorted,
    FrontToBack,
    BackToFront,
}

#[derive(Default)]
pub struct PipelineSorter
{
    // track/sort by average depth?
    pipeline_draws: HashMap<PipelineKey, Vec<Draw>>,
}
impl PipelineSorter
{
    pub fn push(&mut self, draw: Draw)
    {
        self.pipeline_draws.entry(draw.pipeline_hash).or_default()
            .push(draw);
    }

    // process all the entries in this sorter, if returned iter is fully consumed, the sorter should be empty after
    #[inline] #[must_use]
    pub fn sort(&mut self) -> SorterIter
    {
        SorterIter
        {
            iter: self.pipeline_draws.drain(),
            direction: SortDirection::Unsorted,
        }
    }

    #[inline] #[must_use] pub fn is_empty(&self) -> bool { self.pipeline_draws.is_empty() }

    #[inline] pub fn clear(&mut self)
    {
        self.pipeline_draws.clear();
    }
}

#[must_use]
fn sort_front_to_back(a: &Draw, b: &Draw) -> Ordering
{
    match a.depth - b.depth
    {
        t if t < 0.0 => Ordering::Less,
        0.0 => Ordering::Equal,
        t if t > 0.0 => Ordering::Greater,

        _ => unreachable!(),
    }
}
#[must_use]
fn sort_back_to_front(a: &Draw, b: &Draw) -> Ordering
{
    match a.depth - b.depth
    {
        t if t < 0.0 => Ordering::Greater,
        0.0 => Ordering::Equal,
        t if t > 0.0 => Ordering::Less,

        _ => unreachable!(),
    }
}

pub struct SorterIter<'s>
{
    iter: Drain<'s, PipelineKey, Vec<Draw>>,
    direction: SortDirection,
}
impl<'s> Iterator for SorterIter<'s>
{
    type Item = (PipelineKey, Vec<Draw>);

    fn next(&mut self) -> Option<Self::Item>
    {
        self.iter.next().map(|(ph, mut draws)|
        {
            match self.direction
            {
                SortDirection::Unsorted => {},
                // stable sort?
                SortDirection::FrontToBack => draws.sort_unstable_by(sort_front_to_back),
                SortDirection::BackToFront => draws.sort_unstable_by(sort_back_to_front),
            }

            (ph, draws)
        })
    }
}