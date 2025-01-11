#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Intersection
{
    None,
    Overlapping,
    EdgesTouching,
    // Fully contained?
}

// TODO: rename these to be more clear of who contains who

pub trait Intersects<T>
{
    fn get_intersection(&self, other: T) -> Intersection;
}

pub trait IsOnOrInside<T>
{
    fn is_on_or_inside(&self, other: T) -> bool;
}

pub enum Facing
{
    Behind,
    On,
    InFront,
}

pub trait GetFacing<T>
{
    fn get_facing(&self, other: T) -> Facing;
}


// TODO: can probably simplify these somewhat