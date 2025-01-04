#[derive(Clone, Copy)]
pub enum Intersection
{
    None,
    Overlapping,
    Contained
}
impl Intersection
{
    pub fn combine_with(&mut self, other: Intersection)
    {
        *self = match other
        {
            Intersection::None => *self,
            Intersection::Overlapping => Intersection::Overlapping,
            Intersection::Contained => match self
            {
                Intersection::Contained => Intersection::Contained,
                _ => Intersection::Overlapping,
            },
        };
    }
}

pub trait Intersects<T>
{
    fn intersects(&self, other: &T) -> Intersection;
}

pub enum Facing
{
    Behind,
    On,
    InFront,
}

pub trait GetFacing<T>
{
    fn get_facing(&self, other: &T) -> Facing;
}