use std::cmp::Ordering;
use std::collections::hash_map::Drain;
use std::collections::HashMap;
use std::sync::Arc;
use arrayvec::ArrayVec;
use glam::Mat4;
use crate::assets::{Geometry, Material, Shader, Texture, MAX_MATERIAL_TEXTURE_BINDINGS};
use crate::pipeline_cache::PipelineHash;

pub struct Draw
{
    pub transform: Mat4,
    pub depth: f32,
    pub mesh_index: u32,
    pub uniform_id: u32,
    pub pipeline_hash: PipelineHash,
    pub geometry: Arc<Geometry>,
    pub material: Arc<Material>,
    pub textures: ArrayVec<Arc<Texture>, MAX_MATERIAL_TEXTURE_BINDINGS>,
    pub vshader: Arc<Shader>,
    pub pshader: Arc<Shader>,
}

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
    pipeline_draws: HashMap<PipelineHash, Vec<Draw>>,
}
impl PipelineSorter
{
    pub fn push(&mut self, draw: Draw)
    {
        self.pipeline_draws.entry(draw.pipeline_hash).or_default()
            .push(draw);
    }

    // process all the entries in this sorter, if returned iter is fully consumed, the sorter should be empty after
    pub fn sort(&mut self) -> SorterIter
    {
        SorterIter
        {
            iter: self.pipeline_draws.drain(),
            direction: SortDirection::Unsorted,
        }
    }

    pub fn is_empty(&self) -> bool { self.pipeline_draws.is_empty() }

    pub fn clear(&mut self)
    {
        self.pipeline_draws.clear();
    }
}

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
    iter: Drain<'s, PipelineHash, Vec<Draw>>,
    direction: SortDirection,
}
impl<'s> Iterator for SorterIter<'s>
{
    type Item = (PipelineHash, Vec<Draw>);

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