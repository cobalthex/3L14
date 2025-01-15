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
    fn other_is_on_or_inside(&self, other: T) -> bool;
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

// The distance from this object's center to another object's center
pub trait CenterDistance<T>
{
    fn center_distance(&self, other: T) -> f32 { self.center_distance_sq(other).sqrt() }
    fn center_distance_sq(&self, other: T) -> f32;
}

// TODO: can probably simplify these somewhat