use std::hash::Hasher;
use std::io::{Read, Seek, SeekFrom, Write};
use metrohash::{MetroHash128, MetroHash64};

pub struct InlineWriteHash<THasher: Hasher, TReadOrWrite>
{
    hasher: THasher, // type param?
    read_or_write: TReadOrWrite,
}
impl<THasher: Hasher + Default, TReadOrWrite> InlineWriteHash<THasher, TReadOrWrite>
{
    pub fn new(read_or_write: TReadOrWrite) -> Self
    {
        Self
        {
            hasher: THasher::default(),
            read_or_write,
        }
    }
}
impl<TReadOrWrite> InlineWriteHash<MetroHash128, TReadOrWrite>
{
    pub fn with_seed(read_or_write: TReadOrWrite, hasher_seed: u64) -> Self
    {
        Self
        {
            hasher: MetroHash128::with_seed(hasher_seed),
            read_or_write,
        }
    }

    pub fn finish(self) -> (u128, TReadOrWrite)
    {
        let (low, high) = self.hasher.finish128();
        let u = ((high as u128) << 64) | low as u128;
        (u, self.read_or_write)
    }
}
impl<TReadOrWrite> InlineWriteHash<MetroHash64, TReadOrWrite>
{
    pub fn with_seed(read_or_write: TReadOrWrite, hasher_seed: u64) -> Self
    {
        Self
        {
            hasher: MetroHash64::with_seed(hasher_seed),
            read_or_write,
        }
    }

    pub fn finish(self) -> (u64, TReadOrWrite)
    {
        (self.hasher.finish(), self.read_or_write)
    }
}
impl<THasher: Hasher, TRead: Read> Read for InlineWriteHash<THasher, TRead>
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>
    {
        let filled = self.read_or_write.read(buf)?;
        self.hasher.write(&buf[0..filled]);
        Ok(filled)
    }
}
impl<THasher: Hasher, TWrite: Write> Write for InlineWriteHash<THasher, TWrite>
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize>
    {
        self.hasher.write(buf);
        self.read_or_write.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()>
    {
        self.read_or_write.flush()
    }
}
impl<THasher: Hasher, TSeek: Seek> Seek for InlineWriteHash<THasher, TSeek>
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64>
    {
        self.read_or_write.seek(pos)
    }
}