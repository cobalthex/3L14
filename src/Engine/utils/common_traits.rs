pub trait AsIterator<'i>
{
    type Item;
    type AsIter: Iterator<Item = Self::Item>;

    fn as_iter(&'i self) -> Self::AsIter;
}