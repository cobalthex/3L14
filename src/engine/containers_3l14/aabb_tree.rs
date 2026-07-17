use bitcode::{Decode, Encode};
use math_3l14::AABB;
use nab_3l14::debug_panic;
use std::fmt::{Debug, Formatter};
use smallvec::{smallvec, SmallVec};
use crate::NodeIndex;

#[derive(Default, Clone, Encode, Decode)]
struct Node
{
    bounds: AABB,
    leaf_index: NodeIndex, // points to index in values list (if a leaf), none otherwise
    parent_index: NodeIndex,
    // N children?
    // note: child nodes are either all empty or all full
    left_child_index: NodeIndex, // points to left child, none if leaf
    right_child_index: NodeIndex, // points to right child, none if leaf
    // height: u16,

    // todo: can union left child with free list index and right child with leaf index
}

#[derive(Clone, Encode, Decode)]
pub struct AabbTree<T>
{
    nodes: Vec<Node>, // TODO: use a free list (and or slot map)
    len: u32, // todo: get from future free list
    root_index: NodeIndex,
    values: Vec<T>,
}
impl<T> AabbTree<T>
{
    #[inline] #[must_use]
    pub fn new() -> Self
    {
        Self
        {
            nodes: Vec::new(),
            len: 0,
            root_index: NodeIndex::none(),
            values: Vec::new(),
        }
    }

    #[inline] #[must_use]
    pub fn len(&self) -> u32 { self.len }

    #[inline(always)] #[must_use] fn node(&self, index: NodeIndex) -> &Node { &self.nodes[index.0 as usize] }
    #[inline(always)] #[must_use] fn node_mut(&mut self, index: NodeIndex) -> &mut Node { &mut self.nodes[index.0 as usize] }

    pub fn insert(&mut self, bounds: AABB, value: T)
    {
        self.values.push(value);
        let values_index = (self.values.len() - 1) as u32;
        let leaf_index = self.alloc_node(Node
        {
            bounds,
            leaf_index: NodeIndex::some(values_index),
            ..Default::default()
        });
        if self.root_index.is_none()
        {
            self.root_index = leaf_index;
            return;
        }

        let sibling_index = self.pick_best_sibling(bounds);

        // create new parent
        let old_parent_index = self.node(sibling_index).parent_index;
        let new_parent_index = self.alloc_node(Node
        {
            bounds: bounds.unioned_with(self.node(sibling_index).bounds),
            leaf_index: NodeIndex::none(),
            parent_index: old_parent_index,
            .. Default::default()
        });

        if old_parent_index.is_some()
        {
            if self.node(old_parent_index).left_child_index == sibling_index
            {
                self.node_mut(old_parent_index).left_child_index = new_parent_index;
            }
            else
            {
                self.node_mut(old_parent_index).right_child_index = new_parent_index;
            }

            self.node_mut(new_parent_index).left_child_index = sibling_index;
            self.node_mut(new_parent_index).right_child_index = leaf_index;
            self.node_mut(sibling_index).parent_index = new_parent_index;
            self.node_mut(leaf_index).parent_index = new_parent_index;
        }
        else
        {
            // sibling was root
            self.node_mut(new_parent_index).left_child_index = sibling_index;
            self.node_mut(new_parent_index).right_child_index = leaf_index;
            self.node_mut(sibling_index).parent_index = new_parent_index;
            self.node_mut(leaf_index).parent_index = new_parent_index;
            self.root_index = new_parent_index;
        }

        let should_rotate = true;
        self.refit_parents(self.node(leaf_index).parent_index, should_rotate);
    }

    fn refit_parents(&mut self, mut node_index: NodeIndex, should_rotate: bool)
    {
        // debug_assert not leaf?
        while node_index.is_some()
        {
            // todo: awkward syntax w/ ref lifetimes
            let node = self.node(node_index);
            let left_child_bounds = self.node(node.left_child_index).bounds;
            let right_child_bounds = self.node(node.right_child_index).bounds;
            self.node_mut(node_index).bounds = left_child_bounds.unioned_with(right_child_bounds);

            if should_rotate
            {
                self.rotate(node_index);
            }

            node_index = self.node_mut(node_index).parent_index;
        }
    }

    pub fn remove(&mut self, bounds: AABB) -> bool
    {
        let leaf_index = self.index_of(bounds);
        if leaf_index.is_none()
        {
            return false;
        }

        if leaf_index == self.root_index
        {
            self.free_node(self.root_index);
            self.root_index = NodeIndex::none();
            return true;
        }

        let leaf = self.node(leaf_index);
        let parent_index = leaf.parent_index;
        let parent = self.node(leaf.parent_index);
        let gparent_index = parent.parent_index;
        let sibling_index =
            if parent.left_child_index == leaf_index { parent.right_child_index }
            else { parent.left_child_index };

        if gparent_index.is_some()
        {
            let gparent = &mut self.node_mut(gparent_index);
            // destroy parent and replace w/ leaf sibling
            if gparent.left_child_index == parent_index
            {
                gparent.left_child_index = sibling_index;
            }
            else
            {
                gparent.right_child_index = sibling_index;
            }

            self.node_mut(sibling_index).parent_index = gparent_index;
            self.free_node(parent_index);

            let should_rotate = true;
            self.refit_parents(gparent_index, should_rotate);
        }
        else
        {
            self.root_index = sibling_index;
            self.node_mut(sibling_index).parent_index = NodeIndex::none();
            self.free_node(parent_index);
        }

        self.free_node(leaf_index);
        return true;
    }

    #[must_use]
    pub fn contains(&self, bounds: AABB) -> bool
    {
        let leaf_index = self.index_of(bounds);
        leaf_index.is_some()
    }

    #[must_use]
    fn index_of(&self, bounds: AABB) -> NodeIndex
    {
        if self.root_index.is_none() { return NodeIndex::none(); }

        let mut stack: SmallVec<[NodeIndex; 16]> = smallvec![self.root_index];
        while let Some(top) = stack.pop()
        {
            let node = self.node(top);
            if !node.bounds.overlaps(bounds)
            {
                continue;
            }

            if node.leaf_index.is_some()
            {
                if node.bounds == bounds
                {
                    return top
                }

                // assert no children?
                continue;
            }

            if node.right_child_index.is_some() { stack.push(node.right_child_index); }
            if node.left_child_index.is_some() { stack.push(node.left_child_index); }
        }

        NodeIndex::none()
    }

    #[inline] #[must_use]
    fn alloc_node(&mut self, node: Node) -> NodeIndex
    {
        debug_assert!(self.nodes.len() < u32::MAX as usize);
        self.len += 1;
        self.nodes.push(node);
        NodeIndex::some((self.nodes.len() - 1) as u32)
    }

    #[inline]
    fn free_node(&mut self, node_index: NodeIndex)
    {
        self.len -= 1;
        // TODO
    }

    #[must_use]
    fn pick_best_sibling(&self, incoming: AABB) -> NodeIndex
    {
        // based on box2D/daabbc3d

        let incoming_area = incoming.surface_area();

        let root = self.node(self.root_index);
        let mut curr_area = root.bounds.surface_area();
        let mut direct_cost = root.bounds.unioned_with(incoming).surface_area();
        let mut inherited_cost = 0.0;

        let mut best_sibling = self.root_index;
        let mut best_cost = direct_cost;

        let mut curr_index = self.root_index;
        let mut curr = self.node(curr_index);
        while curr.leaf_index.is_none()
        {
            let cost = direct_cost + inherited_cost;
            if cost < best_cost
            {
                best_cost = cost;
                best_sibling = curr_index;
            }

            inherited_cost += direct_cost - curr_area;

            let left = self.node(curr.left_child_index);
            let mut left_lower_bound = f32::MAX;
            let mut left_area = 0.0;
            let left_direct_cost = left.bounds.unioned_with(incoming).surface_area();
            if left.leaf_index.is_some()
            {
                let left_cost = left_direct_cost + inherited_cost;
                if  left_cost < best_cost
                {
                    best_cost = left_cost;
                    best_sibling = curr.left_child_index;
                }
            }
            else
            {
                left_area = left.bounds.surface_area();
                left_lower_bound = inherited_cost + left_direct_cost + f32::min(0.0, incoming_area - left_area);
            }

            // TODO: dedupe this
            let right = self.node(curr.right_child_index);
            let mut right_lower_bound = f32::MAX;
            let mut right_area = 0.0;
            let right_direct_cost = right.bounds.unioned_with(incoming).surface_area();
            if right.leaf_index.is_some()
            {
                let right_cost = right_direct_cost + inherited_cost;
                if  right_cost < best_cost
                {
                    best_cost = right_cost;
                    best_sibling = curr.right_child_index;
                }
            }
            else
            {
                right_area = right.bounds.surface_area();
                right_lower_bound = inherited_cost + right_direct_cost + f32::min(0.0, incoming_area - right_area);
            }

            if (left.leaf_index.is_some() && right.leaf_index.is_some()) ||
                (best_cost <= left_lower_bound && best_cost <= right_lower_bound)
            {
                break;
            }

            if left_lower_bound == right_lower_bound &&
                left.leaf_index.is_none()
            {
                debug_assert!(left_lower_bound < f32::MAX);

                // no clear winner, use centroid distance
                let incoming_center = incoming.centroid();
                let left_dist = left.bounds.centroid() - incoming_center;
                let right_dist = right.bounds.centroid() - incoming_center;
                left_lower_bound = left_dist.length_squared();
                right_lower_bound = right_dist.length_squared();
            }

            if left_lower_bound < right_lower_bound &&
                left.leaf_index.is_none()
            {
                curr_index = curr.left_child_index;
                curr = left;
                curr_area = left_area;
                direct_cost = left_direct_cost;
            }
            else
            {
                curr_index = curr.right_child_index;
                curr = right;
                curr_area = right_area;
                direct_cost = right_direct_cost;
            }
        }

        best_sibling
    }
    
    // left/right rotate the node, if imbalanced
    fn rotate(&mut self, rotate_root: NodeIndex)
    {
        if rotate_root.is_none()
        {
            // debug_panic?
            debug_panic!("Tried to rotate an invalid tree");
            return;
        }

        let left = self.node(rotate_root).left_child_index;
        let right = self.node(rotate_root).right_child_index;

        if left.is_none() || right.is_none()
        {
            return;
        }

        let left_node = self.node(left);
        let right_node = self.node(right);

        let left_is_leaf = left_node.leaf_index.is_some();
        let right_is_leaf = right_node.leaf_index.is_some();

        if !left_is_leaf
        {
            debug_assert!(left_node.left_child_index.is_some() && left_node.right_child_index.is_some());
        }
        if !right_is_leaf
        {
            debug_assert!(right_node.left_child_index.is_some() && right_node.right_child_index.is_some());
        }

        let left_surface_area = left_node.bounds.surface_area();
        let right_surface_area = right_node.bounds.surface_area();
        let current_cost = left_surface_area + right_surface_area;

        // AABB parameter is the new bounds of the subtree after the rotation.
        #[derive(Copy, Clone)]
        enum Rotation
        {
            None,
            Left_RightLeft(AABB),
            Left_RightRight(AABB),
            Right_LeftLeft(AABB),
            Right_LeftRight(AABB),
        }

        let mut best_rotation = Rotation::None;
        let mut best_cost = current_cost;

        let mut consider = |rotation: Rotation, cost: f32|
        {
            if cost < best_cost
            {
                best_cost = cost;
                best_rotation = rotation;
            }
        };

        match (left_is_leaf, right_is_leaf)
        {
            (true, true) => {}
            (true, false) =>
            {
                let right_left = right_node.left_child_index;
                let right_right = right_node.right_child_index;
                if right_left.is_none() || right_right.is_none()
                {
                    debug_panic!("Tried to rotate a leaf node with no children:");
                    return;
                }

                let promote_right_left_bounds = left_node.bounds.unioned_with(self.node(right_right).bounds);
                let promote_right_right_bounds = left_node.bounds.unioned_with(self.node(right_left).bounds);

                consider(Rotation::Left_RightLeft(promote_right_left_bounds), left_surface_area + promote_right_left_bounds.surface_area());
                consider(Rotation::Left_RightRight(promote_right_right_bounds), left_surface_area + promote_right_right_bounds.surface_area());
            }
            (false, true) =>
            {
                let left_left = left_node.left_child_index;
                let left_right = left_node.right_child_index;
                debug_assert!(left_left.is_some() && left_right.is_some());
                if left_left.is_none() || left_right.is_none()
                {
                    return;
                }

                let promote_left_left_bounds = right_node.bounds.unioned_with(self.node(left_right).bounds);
                let promote_left_right_bounds = right_node.bounds.unioned_with(self.node(left_left).bounds);

                consider(Rotation::Right_LeftLeft(promote_left_left_bounds), right_surface_area + promote_left_left_bounds.surface_area());
                consider(Rotation::Right_LeftRight(promote_left_right_bounds), right_surface_area + promote_left_right_bounds.surface_area());
            }
            (false, false) =>
            {
                let left_left = left_node.left_child_index;
                let left_right = left_node.right_child_index;
                let right_left = right_node.left_child_index;
                let right_right = right_node.right_child_index;
                debug_assert!(left_left.is_some() && left_right.is_some());
                debug_assert!(right_left.is_some() && right_right.is_some());
                if left_left.is_none() || left_right.is_none() || right_left.is_none() || right_right.is_none()
                {
                    return;
                }

                let promote_right_left_bounds = left_node.bounds.unioned_with(self.node(right_right).bounds);
                let promote_right_right_bounds = left_node.bounds.unioned_with(self.node(right_left).bounds);
                let promote_left_left_bounds = right_node.bounds.unioned_with(self.node(left_right).bounds);
                let promote_left_right_bounds = right_node.bounds.unioned_with(self.node(left_left).bounds);

                consider(Rotation::Left_RightLeft(promote_right_left_bounds), left_surface_area + promote_right_left_bounds.surface_area());
                consider(Rotation::Left_RightRight(promote_right_right_bounds), left_surface_area + promote_right_right_bounds.surface_area());
                consider(Rotation::Right_LeftLeft(promote_left_left_bounds), right_surface_area + promote_left_left_bounds.surface_area());
                consider(Rotation::Right_LeftRight(promote_left_right_bounds), right_surface_area + promote_left_right_bounds.surface_area());
            }
        }

        if best_cost >= current_cost
        {
            return;
        }

        match best_rotation
        {
            Rotation::None => {}
            Rotation::Left_RightLeft(new_right_bounds) =>
            {
                let right_left = right_node.left_child_index;

                self.node_mut(rotate_root).left_child_index = right_left;
                self.node_mut(right).left_child_index = left;
                self.node_mut(right).bounds = new_right_bounds;

                self.node_mut(left).parent_index = right;
                self.node_mut(right_left).parent_index = rotate_root;
            }
            Rotation::Left_RightRight(new_right_bounds) =>
            {
                let right_right = right_node.right_child_index;

                self.node_mut(rotate_root).left_child_index = right_right;
                self.node_mut(right).right_child_index = left;
                self.node_mut(right).bounds = new_right_bounds;

                self.node_mut(left).parent_index = right;
                self.node_mut(right_right).parent_index = rotate_root;
            }
            Rotation::Right_LeftLeft(new_left_bounds) =>
            {
                let left_left = left_node.left_child_index;

                self.node_mut(rotate_root).right_child_index = left_left;
                self.node_mut(left).left_child_index = right;
                self.node_mut(left).bounds = new_left_bounds;

                self.node_mut(right).parent_index = left;
                self.node_mut(left_left).parent_index = rotate_root;
            }
            Rotation::Right_LeftRight(new_left_bounds) =>
            {
                let left_right = left_node.right_child_index;

                self.node_mut(rotate_root).right_child_index = left_right;
                self.node_mut(left).right_child_index = right;
                self.node_mut(left).bounds = new_left_bounds;

                self.node_mut(right).parent_index = left;
                self.node_mut(left_right).parent_index = rotate_root;
            }
        }
    }

    #[must_use]
    pub fn iter_overlapping(&self, aabb: AABB) -> AabbTreeIterOverlapping<T>
    {
        AabbTreeIterOverlapping
        {
            tree: &self,
            aabb,
            stack: if self.root_index.is_some() { smallvec![self.root_index] } else { SmallVec::new() },
        }
    }

    // Re-order the tree for more efficient traversal
    pub fn repack(&mut self)
    {
        // sort as DFS, as searches likely traverse down specific subtrees

        if self.root_index.is_none()
        {
            debug_assert!(self.len() == 0);
            return;
        }

        let mut nodes = Vec::with_capacity(self.len() as usize);

        // TODO: sort values

        let mut stack = vec![(NodeIndex::none(), 0, self.root_index)];
        while let Some((parent_index, sibling_index, node_index)) = stack.pop()
        {
            let hydrated = self.node(node_index);

            let new_index = NodeIndex::some(nodes.len() as u32);
            nodes.push(Node
            {
                bounds: hydrated.bounds,
                leaf_index: hydrated.leaf_index,
                parent_index,
                left_child_index: NodeIndex::none(),
                right_child_index: NodeIndex::none(),
            });

            if parent_index.is_some()
            {
                let pardrated = &mut nodes[parent_index.0 as usize];
                match sibling_index
                {
                    0 => pardrated.left_child_index = new_index,
                    1 => pardrated.right_child_index = new_index,
                    _ => panic!("There are only two siblings per level"),
                }
            }

            // if hydrated.leaf_index.is_some()
            // {
            //     // todo: set sort index for values here
            // }
            if hydrated.right_child_index.is_some() { stack.push((new_index, 1, hydrated.right_child_index)); }
            if hydrated.left_child_index.is_some() { stack.push((new_index, 0, hydrated.left_child_index)); }
        }

        self.nodes = nodes; // in place sort?
        self.root_index = NodeIndex::some(0);
        self.values.shrink_to_fit(); // necessary if this is being used before serializing?
    }

    // Map the values of this aabb-tree to a different type without changing the hierarchy
    #[must_use]
    pub fn map<U>(mut self, f: impl FnMut(T) -> U) -> AabbTree<U>
    {
        AabbTree
        {
            len: self.len,
            nodes: self.nodes, // shrink to fit?
            root_index: self.root_index,
            values: self.values.drain(..).map(f).collect(),
        }
    }
}
impl<T: Debug> Debug for AabbTree<T>
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.write_fmt(format_args!("AabbTree ({} nodes)", self.len()))?;
        if self.root_index.is_none()
        {
            return Ok(());
        }

        let mut stack = vec![(0, '^', self.root_index)];
        while let Some((depth, l_r, node)) = stack.pop()
        {
            if f.alternate()
            {
                f.write_fmt(format_args!("\n{:3}  ", node.0))?;
            }
            else
            {
                f.write_str("\n  ")?;
            }

            for i in 0..depth
            {
                f.write_str([" ┗━ ", "━━ "][i.min(1)])?;
            }
            let hydrated = self.node(node);
            f.write_fmt(format_args!("[{l_r}] {:?}", hydrated.bounds))?;
            if hydrated.leaf_index.is_some()
            {
                f.write_str(" (Leaf) value: ")?;
                Debug::fmt(&self.values[hydrated.leaf_index.0 as usize], f)?;
            }
            if hydrated.right_child_index.is_some() { stack.push((depth + 1, 'R', hydrated.right_child_index)); }
            if hydrated.left_child_index.is_some() { stack.push((depth + 1, 'L', hydrated.left_child_index)); }
        }

        Ok(())
    }
}

pub struct AabbTreeIterOverlapping<'t, T>
{
    tree: &'t AabbTree<T>,
    aabb: AABB,
    stack: SmallVec<[NodeIndex; 16]>, // TODO: determine a good size based on usage?
}
impl<'t, T> Iterator for AabbTreeIterOverlapping<'t, T>
{
    type Item = (AABB, &'t T);
    fn next(&mut self) -> Option<Self::Item>
    {
        while let Some(top) = self.stack.pop()
        {
            let node = self.tree.node(top);
            if !node.bounds.overlaps(self.aabb)
            {
                continue;
            }

            if node.leaf_index.is_some()
            {
                return Some((node.bounds, &self.tree.values[node.leaf_index.0 as usize]));
            }

            if node.right_child_index.is_some() { self.stack.push(node.right_child_index); }
            if node.left_child_index.is_some() { self.stack.push(node.left_child_index); }
        }

        None
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use glam::Vec3;

    #[test]
    fn basic()
    {
        let mut tree = AabbTree::new();

        let a = AABB::new(Vec3::splat(1.0), Vec3::splat(2.0));
        tree.insert(a, 'a');

        let b = AABB::new(Vec3::splat(10.0), Vec3::splat(15.0));
        tree.insert(b, 'b');

        let c = AABB::new(Vec3::splat(12.0), Vec3::splat(13.0));
        tree.insert(c, 'c');

        let d = AABB::new(Vec3::splat(3.0), Vec3::splat(4.0));
        tree.insert(d, 'd');

        let e = AABB::new(Vec3::splat(3.5), Vec3::splat(3.8));
        tree.insert(e, 'e');

        assert!(tree.contains(a));
        assert!(tree.contains(b));
        assert!(tree.contains(c));
        assert!(tree.contains(d));
        assert!(tree.contains(e));
        assert!(!tree.contains(AABB::empty()));

        println!("{tree:?}\n");

        let test = AABB::new(Vec3::splat(3.0), Vec3::splat(11.0));
        let overlapping: Box<[_]> = tree.iter_overlapping(test).collect();
        assert_eq!(overlapping.len(), 3);

        assert_eq!(overlapping[0].0, d);
        assert_eq!(*overlapping[0].1, 'd');

        assert_eq!(overlapping[1].0, e);
        assert_eq!(*overlapping[1].1, 'e');

        assert_eq!(overlapping[2].0, b);
        assert_eq!(*overlapping[2].1, 'b');

        // TODO: test other bounds
    }

    #[test]
    fn remove()
    {
        let mut tree = AabbTree::new();

        let a = AABB::new(Vec3::splat(1.0), Vec3::splat(2.0));
        tree.insert(a, 'a');

        let b = AABB::new(Vec3::splat(10.0), Vec3::splat(15.0));
        tree.insert(b, 'b');

        let c = AABB::new(Vec3::splat(12.0), Vec3::splat(13.0));
        tree.insert(c, 'c');

        println!("{tree:#?}\n");

        assert!(tree.remove(b));
        println!("Removed b: {tree:#?}\n");
        let overlapping: Box<[_]> = tree.iter_overlapping(AABB::MIN_MAX).collect();
        assert_eq!(overlapping.len(), 2);

        assert_eq!(overlapping[0].0, a);
        assert_eq!(*overlapping[0].1, 'a');

        assert_eq!(overlapping[1].0, c);
        assert_eq!(*overlapping[1].1, 'c');

        assert!(!tree.remove(b));
        println!("Removed b (no-op): {tree:#?}\n");
        let overlapping: Box<[_]> = tree.iter_overlapping(AABB::MIN_MAX).collect();
        assert_eq!(overlapping.len(), 2);

        assert_eq!(overlapping[0].0, a);
        assert_eq!(*overlapping[0].1, 'a');

        assert_eq!(overlapping[1].0, c);
        assert_eq!(*overlapping[1].1, 'c');

        assert!(tree.remove(a));
        println!("Removed a: {tree:#?}\n");
        let overlapping: Box<[_]> = tree.iter_overlapping(AABB::MIN_MAX).collect();
        assert_eq!(overlapping.len(), 1);

        assert_eq!(overlapping[0].0, c);
        assert_eq!(*overlapping[0].1, 'c');

        assert!(tree.remove(c));
        println!("Removed c: {tree:#?}\n");
        let overlapping: Box<[_]> = tree.iter_overlapping(AABB::MIN_MAX).collect();
        assert_eq!(overlapping.len(), 0);
    }

    #[test]
    fn rotate()
    {
        let mut tree = AabbTree::new();
        tree.values.extend_from_slice(&["left", "right-left", "right-right"]);
        tree.nodes.extend_from_slice(&[
            Node
            {
                bounds: AABB::new(Vec3::splat(0.0), Vec3::splat(101.0)),
                left_child_index: NodeIndex(1),
                right_child_index: NodeIndex(2),
                .. Node::default()
            },
            Node
            {
                bounds: AABB::new(Vec3::splat(0.0), Vec3::splat(1.0)),
                parent_index: NodeIndex(0),
                leaf_index: NodeIndex(0),
                .. Node::default()
            },
            Node
            {
                bounds: AABB::new(Vec3::splat(2.0), Vec3::splat(101.0)),
                parent_index: NodeIndex(0),
                left_child_index: NodeIndex(3),
                right_child_index: NodeIndex(4),
                .. Node::default()
            },
            Node
            {
                bounds: AABB::new(Vec3::splat(2.0), Vec3::splat(3.0)),
                parent_index: NodeIndex(2),
                leaf_index: NodeIndex(1),
                .. Node::default()
            },
            Node
            {
                bounds: AABB::new(Vec3::splat(100.0), Vec3::splat(101.0)),
                parent_index: NodeIndex(2),
                leaf_index: NodeIndex(2),
                .. Node::default()
            },
        ]);
        tree.len = 5;
        tree.root_index = NodeIndex(0);

        println!("pre: {tree:#?}");
        tree.rotate(NodeIndex(0));
        println!("post: {tree:#?}");

        assert_eq!(tree.nodes[0].left_child_index, NodeIndex(4));
        assert_eq!(tree.nodes[0].right_child_index, NodeIndex(2));

        assert_eq!(tree.nodes[2].left_child_index, NodeIndex(3));
        assert_eq!(tree.nodes[2].right_child_index, NodeIndex(1));
        assert_eq!(tree.nodes[2].parent_index, NodeIndex(0));
        assert_eq!(tree.nodes[2].bounds, AABB::new(Vec3::splat(0.0), Vec3::splat(3.0)));

        assert_eq!(tree.nodes[4].parent_index, NodeIndex(0));
        assert_eq!(tree.nodes[1].parent_index, NodeIndex(2));
        assert_eq!(tree.nodes[3].parent_index, NodeIndex(2));
        assert_eq!(tree.nodes[0].bounds, AABB::new(Vec3::splat(0.0), Vec3::splat(101.0)));
    }
}
