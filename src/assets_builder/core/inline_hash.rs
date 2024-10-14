use std::hash::Hasher;
use std::io::{Read, Seek, SeekFrom, Write};
use metrohash::MetroHash128;

pub struct InlineHash<TReadOrWrite>
{
    hasher: MetroHash128, // type param?
    read_or_write: TReadOrWrite,
}
impl<TReadOrWrite> InlineHash<TReadOrWrite>
{
    pub fn new(read_or_write: TReadOrWrite) -> Self
    {
        Self
        {
            hasher: MetroHash128::new(),
            read_or_write,
        }
    }

    pub fn with_seed(read_or_write: TReadOrWrite, hasher_seed: u64) -> Self
    {
        Self
        {
            hasher: MetroHash128::with_seed(hasher_seed),
            read_or_write,
        }
    }

    pub fn finish(self) -> u128
    {
        let (low, high) = self.hasher.finish128();
        (high << 64) as u128 | low as u128
    }
}
impl<TRead: Read> Read for InlineHash<TRead>
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>
    {
        let filled = self.read_or_write.read(buf)?;
        self.hasher.write(&buf[0..filled]);
        Ok(filled)
    }
}
impl<TWrite: Write> Write for InlineHash<TWrite>
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
impl<TSeek: Seek> Seek for InlineHash<TSeek>
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64>
    {
        self.read_or_write.seek(pos)
    }
}