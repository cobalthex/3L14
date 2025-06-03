use math_3l14::AABB;

const PAGE_SIZE: u8 = 8;

enum RStarTreeNodeValue
{
    Leaf(usize), // index in values table
    Inner(usize),
}

struct RStarTreeNode
{
    bounds: AABB,
    value: RStarTreeNodeValue,
}

struct RStarTreePage
{
    entries: [RStarTreeNode; PAGE_SIZE as usize],
    entry_count: u8,
}

pub struct RStarTree//<T>
{
    hierarchy: Vec<RStarTreeNode>,
    // values: slotmap?
}

//ObjectPoolOwned?
// free list and active list swap entries