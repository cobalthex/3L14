use math_3l14::AABB;
use std::fmt::{Debug, Formatter, Write};
use smallvec::{smallvec, SmallVec};
use crate::NodeIndex;

#[derive(Default)]
struct Node
{
    bounds: AABB,
    leaf_index: NodeIndex, // points to index in values list
    parent_index: NodeIndex,
    // N children?
    left_child_index: NodeIndex,
    right_child_index: NodeIndex,
}

// AABBTree ?
pub struct AabbTree<T>
{
    nodes: Vec<Node>, // TODO: use a free list (and or slot map)
    len: usize, // todo: get from future free list
    root_index: NodeIndex,
    values: Vec<T>,
}
impl<T> AabbTree<T>
{
    #[inline] #[must_use]
    pub fn new() -> Self
    {
        AabbTree
        {
            nodes: Vec::new(),
            len: 0,
            root_index: NodeIndex::none(),
            values: Vec::new(),
        }
    }

    #[inline] #[must_use]
    pub fn len(&self) -> usize { self.len }

    pub fn insert(&mut self, bounds: AABB, value: T)
    {
        self.values.push(value);
        let values_index = self.values.len() - 1;
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
        let old_parent_index = self.nodes[sibling_index.0].parent_index;
        let new_parent_index = self.alloc_node(Node
        {
            bounds: bounds.unioned_with(self.nodes[sibling_index.0].bounds),
            leaf_index: NodeIndex::none(),
            parent_index: old_parent_index,
            .. Default::default()
        });

        if old_parent_index.is_some()
        {
            if self.nodes[old_parent_index.0].left_child_index == sibling_index
            {
                self.nodes[old_parent_index.0].left_child_index = new_parent_index;
            }
            else
            {
                self.nodes[old_parent_index.0].right_child_index = new_parent_index;
            }

            self.nodes[new_parent_index.0].left_child_index = sibling_index;
            self.nodes[new_parent_index.0].right_child_index = leaf_index;
            self.nodes[sibling_index.0].parent_index = new_parent_index;
            self.nodes[leaf_index.0].parent_index = new_parent_index;
        }
        else
        {
            // sibling was root
            self.nodes[new_parent_index.0].left_child_index = sibling_index;
            self.nodes[new_parent_index.0].right_child_index = leaf_index;
            self.nodes[sibling_index.0].parent_index = new_parent_index;
            self.nodes[leaf_index.0].parent_index = new_parent_index;
            self.root_index = new_parent_index;
        }

        self.refit_parents(self.nodes[leaf_index.0].parent_index);
    }

    fn refit_parents(&mut self, mut node_index: NodeIndex)
    {
        // debug_assert not leaf?
        while node_index.is_some()
        {
            // todo: awkward syntax w/ ref lifetimes
            let node = &self.nodes[node_index.0];
            let left_child_bounds = self.nodes[node.left_child_index.0].bounds;
            let right_child_bounds = self.nodes[node.right_child_index.0].bounds;
            let mut node_mut = &mut self.nodes[node_index.0];
            node_mut.bounds = left_child_bounds.unioned_with(right_child_bounds);

            // if should_rotate
            {
                // rotate
            }

            node_index = node_mut.parent_index;
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

        let leaf = &self.nodes[leaf_index.0];
        let parent_index = leaf.parent_index;
        let parent = &self.nodes[leaf.parent_index.0];
        let gparent_index = parent.parent_index;
        let sibling_index =
            if parent.left_child_index == leaf_index { parent.right_child_index }
            else { parent.left_child_index };

        if gparent_index.is_some()
        {
            println!("removed {:?}", &parent.bounds);
            let gparent = &mut self.nodes[gparent_index.0];
            // destroy parent and replace w/ leaf sibling
            if gparent.left_child_index == parent_index
            {
                gparent.left_child_index = sibling_index;
            }
            else
            {
                gparent.right_child_index = sibling_index;
            }

            self.nodes[sibling_index.0].parent_index = gparent_index;
            self.free_node(parent_index);

            self.refit_parents(gparent_index);
        }
        else
        {
            self.root_index = sibling_index;
            self.nodes[sibling_index.0].parent_index = NodeIndex::none();
            self.free_node(parent_index);
        }

        self.free_node(leaf_index);
        return true;
    }

    pub fn contains(&self, bounds: AABB) -> bool
    {
        let leaf_index = self.index_of(bounds);
        leaf_index.is_some()
    }

    #[must_use]
    fn index_of(&self, bounds: AABB) -> NodeIndex
    {
        if self.root_index.is_none() { return NodeIndex::none(); }

        let mut stack: SmallVec<[usize; 16]> = smallvec![self.root_index.0];
        while let Some(top) = stack.pop()
        {
            let node = &self.nodes[top];
            if !node.bounds.overlaps(bounds)
            {
                continue;
            }

            if node.leaf_index.is_some()
            {
                if node.bounds == bounds
                {
                    return NodeIndex::some(top);
                }

                // assert no children?
                continue;
            }

            if node.right_child_index.is_some() { stack.push(node.right_child_index.0); }
            if node.left_child_index.is_some() { stack.push(node.left_child_index.0); }
        }

        NodeIndex::none()
    }

    #[inline] #[must_use]
    fn alloc_node(&mut self, node: Node) -> NodeIndex
    {
        self.len += 1;
        self.nodes.push(node);
        NodeIndex::some(self.nodes.len() - 1)
    }

    #[inline]
    fn free_node(&mut self, node: NodeIndex)
    {
        self.len -= 1;
        // TODO
    }

    #[must_use]
    fn pick_best_sibling(&self, incoming: AABB) -> NodeIndex
    {
        // code based on defold-daabbcc based on erin catto presentation

        let incoming_area = incoming.surface_area();

        let root = &self.nodes[self.root_index.0];
        let mut curr_area = root.bounds.surface_area();
        let mut direct_cost = root.bounds.unioned_with(incoming).surface_area();
        let mut inherited_cost = 0.0;

        let mut best_sibling = self.root_index;
        let mut best_cost = direct_cost;

        let mut curr_index = self.root_index;
        let mut curr = &self.nodes[curr_index.0];
        while curr.leaf_index.is_none()
        {
            let cost = direct_cost + inherited_cost;
            if cost < best_cost
            {
                best_cost = cost;
                best_sibling = curr_index;
            }

            inherited_cost += direct_cost - curr_area;

            let left = &self.nodes[curr.left_child_index.0];
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
            let right = &self.nodes[curr.right_child_index.0];
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
                let incoming_center = incoming.center();
                let left_dist = left.bounds.center() - incoming_center;
                let right_dist = right.bounds.center() - incoming_center;
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

    fn rotate(&mut self)
    {
        // TODO
    }

    #[must_use]
    pub fn iter_overlapping(&self, aabb: AABB) -> AabbTreeIterOverlapping<T>
    {
        AabbTreeIterOverlapping
        {
            tree: &self,
            aabb,
            stack: if self.root_index.is_some() { smallvec![self.root_index.0] } else { SmallVec::new() },
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

        let mut queue = vec![(0, '^', self.root_index)];
        while let Some((depth, l_r, node)) = queue.pop()
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
            let hydrated = &self.nodes[node.0];
            f.write_fmt(format_args!("[{l_r}] {:?}", hydrated.bounds))?;
            if hydrated.leaf_index.is_some()
            {
                f.write_str(" (Leaf) value: ");
                Debug::fmt(&self.values[hydrated.leaf_index.0], f)?;
            }
            if hydrated.right_child_index.is_some() { queue.push((depth + 1, 'R', hydrated.right_child_index)); }
            if hydrated.left_child_index.is_some() { queue.push((depth + 1, 'L', hydrated.left_child_index)); }
        }

        Ok(())
    }
}

struct AabbTreeIterOverlapping<'t, T>
{
    tree: &'t AabbTree<T>,
    aabb: AABB,
    stack: SmallVec<[usize; 16]>, // TODO: determine a good size based on usage?
}
impl<'t, T> Iterator for AabbTreeIterOverlapping<'t, T>
{
    type Item = (AABB, &'t T);
    fn next(&mut self) -> Option<Self::Item>
    {
        while let Some(top) = self.stack.pop()
        {
            let node = &self.tree.nodes[top];
            if !node.bounds.overlaps(self.aabb)
            {
                continue;
            }

            if node.leaf_index.is_some()
            {
                return Some((node.bounds, &self.tree.values[node.leaf_index.0]));
            }

            if node.right_child_index.is_some() { self.stack.push(node.right_child_index.0); }
            if node.left_child_index.is_some() { self.stack.push(node.left_child_index.0); }
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
}
