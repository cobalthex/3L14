#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Intersection
{
    None,
    Overlapping,
    FullyContained,
    // FullyContains?
}

// TODO: can probably simplify these somewhat

// TODO: rename these to be more clear of who contains who

// TODO: problematic?
pub trait Intersects<T>
{
    fn get_intersection(&self, other: T) -> Intersection;
}

pub trait IsOnOrInside<T>
{
    fn rhs_is_on_or_inside(&self, other: T) -> bool;
}

#[derive(Debug)]
pub enum Facing
{
    Behind,
    On,
    InFront,
}

// todo: better name
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

pub trait CanSee<TOther>
{
    fn can_see(&self, other: TOther) -> bool; // can this object see 'other'. This should return true for anything partially or fully visible
}