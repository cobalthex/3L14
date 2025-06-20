use math_3l14::AABB;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fmt::{Debug, Formatter, Write};

// move to shared location?
#[derive(Copy, Clone, PartialEq, Eq)]
struct NodeIndex(pub usize);
impl NodeIndex
{
    const NULL_BIT: usize = 1 << (usize::BITS - 1);

    #[inline] #[must_use] pub const fn none() -> Self { Self(Self::NULL_BIT) }
    #[inline] #[must_use] pub const fn some(n: usize) -> Self { Self(n & (Self::NULL_BIT - 1)) } // assert < NULL_BIT?

    #[inline] #[must_use] pub const fn is_some(self) -> bool { (self.0 & Self::NULL_BIT) == 0 }
    #[inline] #[must_use] pub const fn is_none(self) -> bool { (self.0 & Self::NULL_BIT) != 0 }

    #[inline] #[must_use]
    pub fn hydrate(self, tree: &AabbTree) -> &Node { &tree.nodes[self.0] }
    #[inline] #[must_use]
    pub fn hydrate_mut(self, tree: &mut AabbTree) -> &mut Node { &mut tree.nodes[self.0] }
}
impl Default for NodeIndex
{
    fn default() -> Self { Self::none() }
}

#[derive(Default)]
struct Node
{
    bounds: AABB,
    leaf: NodeIndex, // index points to values list
    parent: NodeIndex,
    // N children?
    left_child: NodeIndex,
    right_child: NodeIndex,
}

pub struct AabbTree
{
    nodes: Vec<Node>, // TODO: use a free list (and or slot map)
    root: NodeIndex,
}
impl AabbTree
{
    #[inline] #[must_use]
    pub fn new() -> Self
    {
        AabbTree
        {
            nodes: Vec::new(),
            root: NodeIndex::none(),
        }
    }

    #[inline] #[must_use]
    pub fn len(&self) -> usize { self.nodes.len() }

    // Is this right?
    #[inline] #[must_use]
    pub fn depth(&self) -> usize { self.nodes.len().div_ceil(2) }

    pub fn insert(&mut self, bounds: AABB)
    {
        let leaf = self.alloc_node(Node
        {
            bounds,
            leaf: NodeIndex::some(0), // TODO
            .. Default::default()
        });
        if self.root.is_none()
        {
            self.root = leaf;
            return;
        }

        let sibling = self.pick_best_sibling(bounds);

        // create new parent
        let old_parent = self.nodes[sibling.0].parent;
        let new_parent = self.alloc_node(Node
        {
            bounds: bounds.unioned_with(self.nodes[sibling.0].bounds),
            leaf: NodeIndex::none(),
            parent: old_parent,
            .. Default::default()
        });

        if old_parent.is_some()
        {
            if self.nodes[old_parent.0].left_child == sibling
            {
                self.nodes[old_parent.0].left_child = new_parent;
            }
            else
            {
                self.nodes[old_parent.0].right_child = new_parent;
            }

            self.nodes[new_parent.0].left_child = sibling;
            self.nodes[new_parent.0].right_child = leaf;
            self.nodes[sibling.0].parent = new_parent;
            self.nodes[leaf.0].parent = new_parent;
        }
        else
        {
            // sibling was root
            self.nodes[new_parent.0].left_child = sibling;
            self.nodes[new_parent.0].right_child = leaf;
            self.nodes[sibling.0].parent = new_parent;
            self.nodes[leaf.0].parent = new_parent;
            self.root = new_parent;
        }

        // re-fit parent AABBs
        let mut index = self.nodes[leaf.0].parent;
        while index.is_some()
        {
            let left_child = self.nodes[index.0].left_child;
            let right_child = self.nodes[index.0].right_child;
            self.nodes[index.0].bounds = self.nodes[left_child.0].bounds.unioned_with(self.nodes[right_child.0].bounds);

            // if should_rotate
            {
                // rotate
            }

            index = self.nodes[index.0].parent;
        }
    }

    pub fn remove(&mut self, bounds: AABB)
    {
        todo!()
    }

    #[inline] #[must_use]
    fn alloc_node(&mut self, node: Node) -> NodeIndex
    {
        self.nodes.push(node);
        NodeIndex::some(self.nodes.len() - 1)
    }

    #[must_use]
    fn pick_best_sibling(&self, incoming: AABB) -> NodeIndex
    {
        // code based on defold-daabbcc based on erin catto presentation

        let incoming_area = incoming.surface_area();

        let root = self.root.hydrate(self);
        let mut curr_area = root.bounds.surface_area();
        let mut direct_cost = root.bounds.unioned_with(incoming).surface_area();
        let mut inherited_cost = 0.0;

        let mut best_sibling = self.root;
        let mut best_cost = direct_cost;

        let mut curr_index = self.root;
        let mut curr = curr_index.hydrate(self);
        while curr.leaf.is_none()
        {
            let cost = direct_cost + inherited_cost;
            if cost < best_cost
            {
                best_cost = cost;
                best_sibling = curr_index;
            }

            inherited_cost += direct_cost - curr_area;

            let left = curr.left_child.hydrate(self);
            let mut left_lower_bound = f32::MAX;
            let mut left_area = 0.0;
            let left_direct_cost = left.bounds.unioned_with(incoming).surface_area();
            if left.leaf.is_some()
            {
                let left_cost = left_direct_cost + inherited_cost;
                if  left_cost < best_cost
                {
                    best_cost = left_cost;
                    best_sibling = curr.left_child;
                }
            }
            else
            {
                left_area = left.bounds.surface_area();
                left_lower_bound = inherited_cost + left_direct_cost + f32::min(0.0, incoming_area - left_area);
            }

            // TODO: dedupe this
            let right = curr.right_child.hydrate(self);
            let mut right_lower_bound = f32::MAX;
            let mut right_area = 0.0;
            let right_direct_cost = right.bounds.unioned_with(incoming).surface_area();
            if right.leaf.is_some()
            {
                let right_cost = right_direct_cost + inherited_cost;
                if  right_cost < best_cost
                {
                    best_cost = right_cost;
                    best_sibling = curr.right_child;
                }
            }
            else
            {
                right_area = right.bounds.surface_area();
                right_lower_bound = inherited_cost + right_direct_cost + f32::min(0.0, incoming_area - right_area);
            }

            if (left.leaf.is_some() && right.leaf.is_some()) ||
                (best_cost <= left_lower_bound && best_cost <= right_lower_bound)
            {
                break;
            }

            if left_lower_bound == right_lower_bound &&
                left.leaf.is_none()
            {
                debug_assert!(left_lower_bound < f32::MAX);

                // no clear winner, use centroid distance
                let incoming_center = incoming.center();
                let left_dist = left.bounds.center() - incoming_center;
                let right_dist = right.bounds.center() - incoming_center;
                left_lower_bound = left_dist.length_squared();
                right_lower_bound = right_dist.length_squared();
            }

            if left_lower_bound < right_lower_bound &&
                left.leaf.is_none()
            {
                curr_index = curr.left_child;
                curr = left;
                curr_area = left_area;
                direct_cost = left_direct_cost;
            }
            else
            {
                curr_index = curr.right_child;
                curr = right;
                curr_area = right_area;
                direct_cost = right_direct_cost;
            }
        }

        best_sibling
    }

    fn rotate(&mut self)
    {
        todo!()
    }

    #[must_use]
    pub fn iter_inside(&self, aabb: AABB) -> AabbTreeIter
    {
        // TODO: need to verify root contains aabb

        AabbTreeIter { tree: self, aabb, curr: self.root.0 }
    }
}
impl Debug for AabbTree
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.write_fmt(format_args!("AabbTree ({} nodes)", self.len()))?;
        if self.root.is_none()
        {
            return Ok(());
        }

        let mut queue = vec![(0, '^', self.root)];
        while let Some((depth, l_r, node)) = queue.pop()
        {
            f.write_str("\n  ")?;
            for i in 0..depth
            {
                f.write_str([" ┗━ ", "━━ "][i.min(1)])?;
            }
            let hydrated = node.hydrate(self);
            f.write_fmt(format_args!("[{l_r}] {:?}{}", hydrated.bounds, ["", " (Leaf)"][hydrated.leaf.is_some() as usize]))?;
            if hydrated.right_child.is_some() { queue.push((depth + 1, 'R', hydrated.right_child)); }
            if hydrated.left_child.is_some() { queue.push((depth + 1, 'L', hydrated.left_child)); }
        }

        Ok(())
    }
}

struct AabbTreeIter<'t>
{
    tree: &'t AabbTree,
    aabb: AABB,
    curr: usize,
}
impl Iterator for AabbTreeIter<'_>
{
    type Item = AABB;
    fn next(&mut self) -> Option<Self::Item>
    {
        if self.curr >= self.tree.len()
        {
            return None;
        }

        let rv = &self.tree.nodes[self.curr];
        if rv.leaf.is_some()
        {
            self.curr = rv.parent.hydrate(self.tree).right_child.0;
            return Some(rv.bounds);
        }

        // todo: loop until found leaf
        todo!()
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    use glam::Vec3;

    #[test]
    fn node_index()
    {
        assert!(NodeIndex::none().is_none());
        assert!(NodeIndex::some(0).is_some());
        assert!(NodeIndex::some(1).is_some());
        assert!(NodeIndex::some((1 << 63) - 1).is_some());
    }

    #[test]
    fn basic()
    {
        let mut tree = AabbTree::new();
        println!("{:?}\n", tree);

        let insert = AABB::new(Vec3::splat(1.0), Vec3::splat(2.0));
        tree.insert(insert);
        println!("{insert:?}\n{:?}\n", tree);

        let insert = AABB::new(Vec3::splat(10.0), Vec3::splat(15.0));
        tree.insert(insert);
        println!("{insert:?}\n{:?}\n", tree);

        let insert = AABB::new(Vec3::splat(12.0), Vec3::splat(13.0));
        tree.insert(insert);
        println!("{insert:?}\n{:?}\n", tree);

        let insert = AABB::new(Vec3::splat(3.0), Vec3::splat(4.0));
        tree.insert(insert);
        println!("{insert:?}\n{:?}\n", tree);

        let insert = AABB::new(Vec3::splat(3.5), Vec3::splat(3.8));
        tree.insert(insert);
        println!("{insert:?}\n{:?}\n", tree);
    }
}