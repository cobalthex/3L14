use bitcode::{Decode, Encode};
use math_3l14::AABB;
use nab_3l14::debug_panic;
use std::fmt::{Debug, Formatter};
use std::assert_matches;
use smallvec::{smallvec, SmallVec};
use crate::NodeIndex;

#[derive(Default, Clone, Encode, Decode)]
struct Node
{
    bounds: AABB,
    parent_index: NodeIndex, // the parent, or none if root
    left_or_nextfree: u32, // left child when internal or next free node when in free list, indeterminate when leaf
    right_or_userdata: u32, // right child when internal, or user data when is leaf, indeterminate when in free list
    height: u16, // 0 = leaf, > 0 = internal
}
impl Node
{
    const LEAF_HEIGHT: u16 = 0;
    #[inline] fn is_leaf(&self) -> bool { self.height == Self::LEAF_HEIGHT }
}

// A BVH using AABBs supporting dynamic insertion and removal of nodes
// Each node can store a u32 for user-data
#[derive(Clone, Encode, Decode)]
pub struct AabbTree
{
    nodes: Vec<Node>, // todo: this should just be an array manually managed
    nodes_free_head: NodeIndex,
    len: u32, // how many active nodes there are
    root_index: NodeIndex,
}
// based on box2D/daabbc3d
impl AabbTree
{
    #[inline] #[must_use]
    pub fn new() -> Self
    {
        Self
        {
            nodes: Vec::with_capacity(16),
            nodes_free_head: NodeIndex::none(),
            len: 0,
            root_index: NodeIndex::none(),
        }
    }

    // Get the number of nodes in the tree
    #[inline] #[must_use]
    pub fn len(&self) -> u32 { self.len }

    #[inline(always)] #[must_use] fn node(&self, index: u32) -> &Node { &self.nodes[index as usize] }
    #[inline(always)] #[must_use] fn node_mut(&mut self, index: u32) -> &mut Node { &mut self.nodes[index as usize] }

    pub fn insert(&mut self, bounds: AABB, value: u32)
    {
        let leaf_node_index = self.alloc_node(Node
        {
            bounds,
            parent_index: NodeIndex::none(),
            left_or_nextfree: 0, // unused
            right_or_userdata: value,
            height: Node::LEAF_HEIGHT,
        });
        if self.root_index.is_none()
        {
            self.root_index = NodeIndex::some(leaf_node_index);
            return;
        }

        let sibling_index = self.pick_best_sibling(bounds);
        let sibling = self.node(sibling_index);

        // create new parent
        let old_parent_index = sibling.parent_index;
        let new_parent_index = self.alloc_node(Node
        {
            bounds: bounds.unioned_with(sibling.bounds),
            height: 1 + sibling.height,
            parent_index: old_parent_index,
            ..Default::default()
        });

        if old_parent_index.is_some()
        {
            if self.node(old_parent_index.0).left_or_nextfree == sibling_index
            {
                self.node_mut(old_parent_index.0).left_or_nextfree = new_parent_index;
            }
            else
            {
                self.node_mut(old_parent_index.0).right_or_userdata = new_parent_index;
            }

            self.node_mut(new_parent_index).left_or_nextfree = sibling_index;
            self.node_mut(new_parent_index).right_or_userdata = leaf_node_index;
            self.node_mut(sibling_index).parent_index = NodeIndex::some(new_parent_index);
            self.node_mut(leaf_node_index).parent_index = NodeIndex::some(new_parent_index);
        }
        else
        {
            // sibling was root
            self.node_mut(new_parent_index).left_or_nextfree = sibling_index;
            self.node_mut(new_parent_index).right_or_userdata = leaf_node_index;
            self.node_mut(sibling_index).parent_index = NodeIndex::some(new_parent_index);
            self.node_mut(leaf_node_index).parent_index = NodeIndex::some(new_parent_index);
            self.root_index = NodeIndex::some(new_parent_index);
        }

        let should_rotate = true;
        self.refit_parents(self.node(leaf_node_index).parent_index, should_rotate);
    }

    fn refit_parents(&mut self, mut node_index: NodeIndex, should_rotate: bool)
    {
        // debug_assert not leaf?
        while node_index.is_some()
        {
            // todo: awkward syntax w/ ref lifetimes
            let node = self.node(node_index.0);
            let left_child = self.node(node.left_or_nextfree);
            let right_child = self.node(node.right_or_userdata);
            *self.node_mut(node_index.0) = Node
            {
                bounds: left_child.bounds.unioned_with(right_child.bounds),
                height: 1 + left_child.height.max(right_child.height),
                .. *node
            };

            if should_rotate
            {
                self.rotate(node_index.0);
            }

            node_index = self.node_mut(node_index.0).parent_index;
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
            self.free_node(self.root_index.0);
            self.root_index = NodeIndex::none();
            return true;
        }

        let leaf = self.node(leaf_index.0);
        let parent_index = leaf.parent_index;
        let parent = self.node(leaf.parent_index.0);
        let gparent_index = parent.parent_index;
        let sibling_index =
            if parent.left_or_nextfree == leaf_index.0 { parent.right_or_userdata }
            else { parent.left_or_nextfree };

        if gparent_index.is_some()
        {
            let gparent = &mut self.node_mut(gparent_index.0);
            // destroy parent and replace w/ leaf sibling
            if gparent.left_or_nextfree == parent_index.0
            {
                gparent.left_or_nextfree = sibling_index;
            }
            else
            {
                gparent.right_or_userdata = sibling_index;
            }

            self.node_mut(sibling_index).parent_index = gparent_index;
            self.free_node(parent_index.0);

            let should_rotate = true;
            self.refit_parents(gparent_index, should_rotate);
        }
        else
        {
            self.root_index = NodeIndex::some(sibling_index);
            self.node_mut(sibling_index).parent_index = NodeIndex::none();
            self.free_node(parent_index.0);
        }

        self.free_node(leaf_index.0);
        true
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

        let mut stack: SmallVec<[_; 16]> = smallvec![self.root_index.0];
        while let Some(top) = stack.pop()
        {
            let node = self.node(top);
            if !node.bounds.overlaps(bounds)
            {
                continue;
            }

            if node.is_leaf()
            {
                if node.bounds == bounds // approx eq?
                {
                    return NodeIndex::some(top)
                }

                // assert no children?
                continue;
            }

            stack.push(node.right_or_userdata);
            stack.push(node.left_or_nextfree);
        }

        NodeIndex::none()
    }

    #[inline] #[must_use]
    fn alloc_node(&mut self, node: Node) -> u32
    {
        self.len += 1;
        if self.nodes_free_head.is_some()
        {
            let index = self.nodes_free_head;
            self.nodes_free_head = NodeIndex::some(self.nodes[index.0 as usize].left_or_nextfree);
            self.nodes[index.0 as usize] = node;
            index.0
        }
        else
        {
            debug_assert!(self.nodes.len() < NodeIndex::MAX as usize);
            self.nodes.push(node);
            (self.nodes.len() - 1) as u32
        }
    }

    #[inline]
    fn free_node(&mut self, node_index: u32)
    {
        debug_assert!(self.len > 0);
        self.len -= 1;
        let old_head = self.nodes_free_head;
        let node = &mut self.nodes[node_index as usize];
        node.height = 0;
        node.left_or_nextfree = old_head.0; // chain into free list
        self.nodes_free_head = NodeIndex::some(node_index);

        // TODO: this needs to clean up the values vec
    }

    #[must_use]
    fn pick_best_sibling(&self, incoming: AABB) -> u32
    {
        debug_assert!(self.root_index.is_some());

        let incoming_area = incoming.surface_area();

        let root = self.node(self.root_index.0);
        let mut curr_area = root.bounds.surface_area();
        let mut direct_cost = root.bounds.unioned_with(incoming).surface_area();
        let mut inherited_cost = 0.0;

        let mut best_sibling = self.root_index.0;
        let mut best_cost = direct_cost;

        let mut curr_index = self.root_index.0;
        let mut curr = self.node(curr_index);
        while !curr.is_leaf()
        {
            let cost = direct_cost + inherited_cost;
            if cost < best_cost
            {
                best_cost = cost;
                best_sibling = curr_index;
            }

            inherited_cost += direct_cost - curr_area;

            let left = self.node(curr.left_or_nextfree);
            let mut left_lower_bound = f32::MAX;
            let mut left_area = 0.0;
            let left_direct_cost = left.bounds.unioned_with(incoming).surface_area();
            if left.is_leaf()
            {
                let left_cost = left_direct_cost + inherited_cost;
                if  left_cost < best_cost
                {
                    best_cost = left_cost;
                    best_sibling = curr.left_or_nextfree;
                }
            }
            else
            {
                left_area = left.bounds.surface_area();
                left_lower_bound = inherited_cost + left_direct_cost + f32::min(0.0, incoming_area - left_area);
            }

            // TODO: dedupe this
            let right = self.node(curr.right_or_userdata);
            let mut right_lower_bound = f32::MAX;
            let mut right_area = 0.0;
            let right_direct_cost = right.bounds.unioned_with(incoming).surface_area();
            if right.is_leaf()
            {
                let right_cost = right_direct_cost + inherited_cost;
                if  right_cost < best_cost
                {
                    best_cost = right_cost;
                    best_sibling = curr.right_or_userdata;
                }
            }
            else
            {
                right_area = right.bounds.surface_area();
                right_lower_bound = inherited_cost + right_direct_cost + f32::min(0.0, incoming_area - right_area);
            }

            if (left.is_leaf() && right.is_leaf()) ||
                (best_cost <= left_lower_bound && best_cost <= right_lower_bound)
            {
                break;
            }

            if left_lower_bound == right_lower_bound &&
                !left.is_leaf()
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
                !left.is_leaf()
            {
                curr_index = curr.left_or_nextfree;
                curr = left;
                curr_area = left_area;
                direct_cost = left_direct_cost;
            }
            else
            {
                curr_index = curr.right_or_userdata;
                curr = right;
                curr_area = right_area;
                direct_cost = right_direct_cost;
            }
        }

        best_sibling
    }

    // left/right rotate the node, if imbalanced
    fn rotate(&mut self, rotate_root_index: u32)
    {
        let rotate_root = self.node(rotate_root_index);
        if rotate_root.height < 2 // no grandchildren, no rotation
        {
            return;
        }

        let left = self.node(rotate_root.left_or_nextfree);
        let right = self.node(rotate_root.right_or_userdata);

        let left_surface_area = left.bounds.surface_area();
        let right_surface_area = right.bounds.surface_area();
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

        match (left.is_leaf(), right.is_leaf())
        {
            (true, true) => { debug_panic!("Both children are leaves, but rotate_root height >= 2"); } // no rotation possible
            (true, false) => // left is leaf, right is internal
            {
                let promote_right_left_bounds = left.bounds.unioned_with(self.node(right.right_or_userdata).bounds);
                let promote_right_right_bounds = left.bounds.unioned_with(self.node(right.left_or_nextfree).bounds);

                consider(Rotation::Left_RightLeft(promote_right_left_bounds), left_surface_area + promote_right_left_bounds.surface_area());
                consider(Rotation::Left_RightRight(promote_right_right_bounds), left_surface_area + promote_right_right_bounds.surface_area());
            }
            (false, true) => // left is internal, right is leaf
            {
                let promote_left_left_bounds = right.bounds.unioned_with(self.node(left.right_or_userdata).bounds);
                let promote_left_right_bounds = right.bounds.unioned_with(self.node(left.left_or_nextfree).bounds);

                consider(Rotation::Right_LeftLeft(promote_left_left_bounds), right_surface_area + promote_left_left_bounds.surface_area());
                consider(Rotation::Right_LeftRight(promote_left_right_bounds), right_surface_area + promote_left_right_bounds.surface_area());
            }
            (false, false) => // neither are leaves, so promote the smallest subtree
            {
                let promote_right_left_bounds = left.bounds.unioned_with(self.node(right.right_or_userdata).bounds);
                let promote_right_right_bounds = left.bounds.unioned_with(self.node(right.left_or_nextfree).bounds);
                let promote_left_left_bounds = right.bounds.unioned_with(self.node(left.right_or_userdata).bounds);
                let promote_left_right_bounds = right.bounds.unioned_with(self.node(left.left_or_nextfree).bounds);

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
                let right_left = right.left_or_nextfree;
                let left_index = rotate_root.left_or_nextfree;
                let right_index = rotate_root.right_or_userdata;

                self.node_mut(rotate_root_index).left_or_nextfree = right_left;
                self.node_mut(right_index).left_or_nextfree = left_index;
                self.node_mut(right_index).bounds = new_right_bounds;

                self.node_mut(left_index).parent_index = NodeIndex::some(right_index);
                self.node_mut(right_left).parent_index = NodeIndex::some(rotate_root_index);
            }
            Rotation::Left_RightRight(new_right_bounds) =>
            {
                let right_right = right.right_or_userdata;
                let left_index = rotate_root.left_or_nextfree;
                let right_index = rotate_root.right_or_userdata;

                self.node_mut(rotate_root_index).left_or_nextfree = right_right;
                self.node_mut(right_index).right_or_userdata = left_index;
                self.node_mut(right_index).bounds = new_right_bounds;

                self.node_mut(left_index).parent_index = NodeIndex::some(right_index);
                self.node_mut(right_right).parent_index = NodeIndex::some(rotate_root_index);
            }
            Rotation::Right_LeftLeft(new_left_bounds) =>
            {
                let left_left = left.left_or_nextfree;
                let left_index = rotate_root.left_or_nextfree;
                let right_index = rotate_root.right_or_userdata;

                self.node_mut(rotate_root_index).right_or_userdata = left_left;
                self.node_mut(left_index).left_or_nextfree = right_index;
                self.node_mut(left_index).bounds = new_left_bounds;

                self.node_mut(right_index).parent_index = NodeIndex::some(left_index);
                self.node_mut(left_left).parent_index = NodeIndex::some(rotate_root_index);
            }
            Rotation::Right_LeftRight(new_left_bounds) =>
            {
                let right_index = rotate_root.right_or_userdata;
                let left_index = rotate_root.left_or_nextfree;
                let left_right = left.right_or_userdata;

                self.node_mut(rotate_root_index).right_or_userdata = left_right;
                self.node_mut(left_index).right_or_userdata = right_index;
                self.node_mut(left_index).bounds = new_left_bounds;

                self.node_mut(right_index).parent_index = NodeIndex::some(left_index);
                self.node_mut(left_right).parent_index = NodeIndex::some(rotate_root_index);
            }
        }
    }

    #[must_use]
    pub fn iter_overlapping(&self, aabb: AABB) -> AabbTreeIterOverlapping
    {
        AabbTreeIterOverlapping
        {
            tree: &self,
            aabb,
            stack: if self.root_index.is_some() { smallvec![self.root_index.0] } else { SmallVec::new() },
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

        let mut stack = vec![(NodeIndex::none(), 0, self.root_index.0)]; // smallvec?
        while let Some((parent_index, sibling_index, node_index)) = stack.pop()
        {
            let hydrated = self.node(node_index);

            let new_index = nodes.len() as u32;
            nodes.push(Node
            {
                bounds: hydrated.bounds,
                left_or_nextfree: 0,
                right_or_userdata: hydrated.right_or_userdata,
                height: hydrated.height,
                parent_index,
            });

            if parent_index.is_some()
            {
                let pardrated = &mut nodes[parent_index.0 as usize];
                match sibling_index
                {
                    0 => pardrated.left_or_nextfree = new_index,
                    1 => pardrated.right_or_userdata = new_index,
                    _ => panic!("There are only two siblings per level"),
                }
            }

            if hydrated.is_leaf()
            {
                // todo: set sort index here
            }
            else
            {
                stack.push((NodeIndex::some(new_index), 1, hydrated.right_or_userdata));
                stack.push((NodeIndex::some(new_index), 0, hydrated.left_or_nextfree));
            }
        }

        self.nodes = nodes; // in place sort?
        self.nodes_free_head = NodeIndex::none();
        self.root_index = NodeIndex::some(0);
    }

    pub fn validate(&self, start: NodeIndex) -> Result<(), (ValidationError, u32)> // bitflags of errors?
    {
        if start.is_none() { return Ok(()); }

        let mut stack: SmallVec<[_; 64]> = smallvec![start.0];
        while let Some(top_index) = stack.pop()
        {
            let top = self.node(top_index);

            if NodeIndex::some(top_index) == self.root_index &&
               top.parent_index.is_some()
            {
                return Err((ValidationError::NodeIsRootButHasParent { parent_index: top.parent_index.0 }, top_index));
            }

            if top.is_leaf()
            {
                return Ok(())
            }

            if top.left_or_nextfree >= self.nodes.len() as u32
            {
                return Err((ValidationError::LeftChildHasInvalidIndex { left_index: top.left_or_nextfree }, top_index));
            }
            if top.right_or_userdata >= self.nodes.len() as u32
            {
                return Err((ValidationError::RightChildHasInvalidIndex { right_index: top.right_or_userdata }, top_index));
            }

            let left = self.node(top.left_or_nextfree);
            let right = self.node(top.right_or_userdata);

            if top.height != 1 + left.height.max(right.height)
            {
                return Err((ValidationError::NodeHeightIsNotOneMoreThanChildren { height: top.height, left_height: left.height, right_height: right.height }, top_index));
            }

            if left.parent_index != NodeIndex::some(top_index)
            {
                return Err((ValidationError::LeftChildMisparented { left_parent_index: left.parent_index }, top_index));
            }
            if right.parent_index != NodeIndex::some(top_index)
            {
                return Err((ValidationError::RightChildMisparented { right_parent_index: right.parent_index }, top_index));
            }

            let children_bounds = left.bounds.unioned_with(right.bounds);
            if top.bounds != children_bounds
            {
                return Err((ValidationError::BoundsDontUnionChildren { bounds: top.bounds, children_bounds }, top_index));
            }

            stack.push(top.left_or_nextfree);
            stack.push(top.right_or_userdata);
        }

        // todo: validate free list
        // todo: validate len + free list count == vector length


        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ValidationError
{
    NodeIsRootButHasParent { parent_index: u32 },
    NodeHeightIsNotOneMoreThanChildren { height: u16, left_height: u16, right_height: u16 },
    LeftChildHasInvalidIndex { left_index: u32 },
    RightChildHasInvalidIndex { right_index: u32 },
    LeftChildMisparented { left_parent_index: NodeIndex },
    RightChildMisparented { right_parent_index: NodeIndex },
    BoundsDontUnionChildren { bounds: AABB, children_bounds: AABB },
}

impl Debug for AabbTree
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.write_fmt(format_args!("AabbTree ({} nodes)", self.len()))?;
        if self.root_index.is_none()
        {
            return Ok(());
        }

        let mut stack: SmallVec<[(u32, _, _); 64]> = smallvec![(0, '^', self.root_index.0)];
        while let Some((depth, l_r, node)) = stack.pop()
        {
            if f.alternate()
            {
                f.write_fmt(format_args!("\n{:3}  ", node))?;
            }
            else
            {
                f.write_str("\n  ")?;
            }

            for i in 0..depth
            {
                f.write_str([" ┗━ ", "━━ "][i.min(1) as usize])?;
            }
            let hydrated = self.node(node);
            f.write_fmt(format_args!("[{l_r}] {:?}", hydrated.bounds))?;
            if hydrated.is_leaf()
            {
                f.write_fmt(format_args!(" (Leaf) user-data: 0x{:x}", hydrated.right_or_userdata))?;
            }
            else
            {
                stack.push((depth + 1, 'R', hydrated.right_or_userdata));
                stack.push((depth + 1, 'L', hydrated.left_or_nextfree));
            }
        }

        Ok(())
    }
}

pub struct AabbTreeIterOverlapping<'t>
{
    tree: &'t AabbTree,
    aabb: AABB,
    stack: SmallVec<[u32; 16]>, // TODO: determine a good size based on usage?
}
impl<'t> Iterator for AabbTreeIterOverlapping<'t>
{
    type Item = (AABB, u32);
    fn next(&mut self) -> Option<Self::Item>
    {
        while let Some(top) = self.stack.pop()
        {
            let node = self.tree.node(top);
            if !node.bounds.overlaps(self.aabb)
            {
                continue;
            }

            if node.is_leaf()
            {
                return Some((node.bounds, node.right_or_userdata));
            }
            else
            {
                self.stack.push(node.right_or_userdata);
                self.stack.push(node.left_or_nextfree);
            }
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
        tree.insert(a, 0);

        let b = AABB::new(Vec3::splat(10.0), Vec3::splat(15.0));
        tree.insert(b, 1);

        let c = AABB::new(Vec3::splat(12.0), Vec3::splat(13.0));
        tree.insert(c, 2);

        let d = AABB::new(Vec3::splat(3.0), Vec3::splat(4.0));
        tree.insert(d, 3);

        let e = AABB::new(Vec3::splat(3.5), Vec3::splat(3.8));
        tree.insert(e, 4);

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
        assert_eq!(overlapping[0].1, 3);

        assert_eq!(overlapping[1].0, e);
        assert_eq!(overlapping[1].1, 4);

        assert_eq!(overlapping[2].0, b);
        assert_eq!(overlapping[2].1, 1);

        // TODO: test other bounds

        tree.validate(tree.root_index).unwrap();
    }

    #[test]
    fn remove()
    {
        let mut tree = AabbTree::new();

        let a = AABB::new(Vec3::splat(1.0), Vec3::splat(2.0));
        tree.insert(a, 0);

        let b = AABB::new(Vec3::splat(10.0), Vec3::splat(15.0));
        tree.insert(b, 1);

        let c = AABB::new(Vec3::splat(12.0), Vec3::splat(13.0));
        tree.insert(c, 2);

        println!("{tree:#?}\n");

        assert!(tree.remove(b));
        println!("Removed b: {tree:#?}\n");
        let overlapping: Box<[_]> = tree.iter_overlapping(AABB::MIN_MAX).collect();
        assert_eq!(overlapping.len(), 2);

        assert_eq!(overlapping[0].0, a);
        assert_eq!(overlapping[0].1, 0);

        assert_eq!(overlapping[1].0, c);
        assert_eq!(overlapping[1].1, 2);

        assert!(!tree.remove(b));
        println!("Removed b (no-op): {tree:#?}\n");
        let overlapping: Box<[_]> = tree.iter_overlapping(AABB::MIN_MAX).collect();
        assert_eq!(overlapping.len(), 2);

        assert_eq!(overlapping[0].0, a);
        assert_eq!(overlapping[0].1, 0);

        assert_eq!(overlapping[1].0, c);
        assert_eq!(overlapping[1].1, 2);

        assert!(tree.remove(a));
        println!("Removed a: {tree:#?}\n");
        let overlapping: Box<[_]> = tree.iter_overlapping(AABB::MIN_MAX).collect();
        assert_eq!(overlapping.len(), 1);

        assert_eq!(overlapping[0].0, c);
        assert_eq!(overlapping[0].1, 2);

        assert!(tree.remove(c));
        println!("Removed c: {tree:#?}\n");
        let overlapping: Box<[_]> = tree.iter_overlapping(AABB::MIN_MAX).collect();
        assert_eq!(overlapping.len(), 0);

        tree.validate(tree.root_index).unwrap();
    }

    #[test]
    fn rotate()
    {
        let mut tree = AabbTree::new();
        tree.nodes.extend_from_slice(&[
            Node
            {
                bounds: AABB::new(Vec3::splat(0.0), Vec3::splat(101.0)),
                left_or_nextfree: 1,
                right_or_userdata: 2,
                height: 2,
                .. Node::default()
            },
            Node
            {
                bounds: AABB::new(Vec3::splat(0.0), Vec3::splat(1.0)),
                parent_index: NodeIndex::some(0),
                right_or_userdata: 0x1337, // userdata
                height: 0, // leaf
                .. Node::default()
            },
            Node
            {
                bounds: AABB::new(Vec3::splat(2.0), Vec3::splat(101.0)),
                parent_index: NodeIndex::some(0),
                left_or_nextfree: 3,
                right_or_userdata: 4,
                height: 1,
                .. Node::default()
            },
            Node
            {
                bounds: AABB::new(Vec3::splat(2.0), Vec3::splat(3.0)),
                parent_index: NodeIndex::some(2),
                right_or_userdata: 0x8008, // userdata
                height: 0, // leaf
                .. Node::default()
            },
            Node
            {
                bounds: AABB::new(Vec3::splat(100.0), Vec3::splat(101.0)),
                parent_index: NodeIndex::some(2),
                right_or_userdata: 0xdeadbeef, // userdata
                height: 0, // leaf
                .. Node::default()
            },
        ]);
        tree.len = 5;
        tree.root_index = NodeIndex::some(0);

        tree.validate(tree.root_index).unwrap();

        println!("pre: {tree:#?}");
        tree.rotate(0);
        println!("post: {tree:#?}");

        assert_eq!(tree.nodes[0].left_or_nextfree, 4);
        assert_eq!(tree.nodes[0].right_or_userdata, 2);

        assert_eq!(tree.nodes[2].left_or_nextfree, 3);
        assert_eq!(tree.nodes[2].right_or_userdata, 1);
        assert_eq!(tree.nodes[2].parent_index, NodeIndex::some(0));
        assert_eq!(tree.nodes[2].bounds, AABB::new(Vec3::splat(0.0), Vec3::splat(3.0)));

        assert_eq!(tree.nodes[4].parent_index, NodeIndex::some(0));
        assert_eq!(tree.nodes[1].parent_index, NodeIndex::some(2));
        assert_eq!(tree.nodes[3].parent_index, NodeIndex::some(2));
        assert_eq!(tree.nodes[0].bounds, AABB::new(Vec3::splat(0.0), Vec3::splat(101.0)));

        tree.validate(tree.root_index).unwrap();
    }

    #[test]
    fn validate()
    {
        let tree =
        {
            let mut tree = AabbTree::new();
            tree.nodes.extend_from_slice(&[
                Node
                {
                    bounds: AABB::new(Vec3::splat(0.0), Vec3::splat(100.0)),
                    left_or_nextfree: 1,
                    right_or_userdata: 2,
                    height: 1,
                    parent_index: NodeIndex::none(),
                },
                Node
                {
                    bounds: AABB::new(Vec3::splat(0.0), Vec3::splat(50.0)),
                    parent_index: NodeIndex::some(0),
                    left_or_nextfree: 0, // unused
                    right_or_userdata: 0, // value index
                    height: 0, // leaf
                },
                Node
                {
                    bounds: AABB::new(Vec3::splat(50.0), Vec3::splat(100.0)),
                    parent_index: NodeIndex::some(0),
                    left_or_nextfree: 0, // unused
                    right_or_userdata: 1, // value index
                    height: 0, // leaf
                }
            ]);
            tree.len = 3;

            tree.root_index = NodeIndex::some(0);
            tree
        };
        tree.validate(tree.root_index).unwrap();

        let mut test = tree.clone();
        test.nodes[0].bounds = AABB::new(Vec3::splat(49.0), Vec3::splat(51.0));
        assert_matches!(test.validate(test.root_index), Err((ValidationError::BoundsDontUnionChildren { .. }, 0)));

        // todo: test all invalid states
    }

}
