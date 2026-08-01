use std::ffi::OsStr;
use std::ops::RangeBounds;

pub trait OsStrUtils
{
    fn starts_with(&self, needle: impl AsRef<OsStr>) -> bool;
    fn ends_with(&self, needle: impl AsRef<OsStr>) -> bool;
    fn position(&self, needle: impl AsRef<OsStr>) -> Option<usize>;

    fn substr(&self, start: usize, end: usize) -> &OsStr; // todo: take a range
}
impl OsStrUtils for OsStr
{
    fn starts_with(&self, needle: impl AsRef<OsStr>) -> bool
    {
        self.as_encoded_bytes().starts_with(needle.as_ref().as_encoded_bytes())
    }
    fn ends_with(&self, needle: impl AsRef<OsStr>) -> bool
    {
        self.as_encoded_bytes().ends_with(needle.as_ref().as_encoded_bytes())
    }
    fn position(&self, needle: impl AsRef<OsStr>) -> Option<usize>
    {
        self.as_encoded_bytes().iter().position(|&b| b == needle.as_ref().as_encoded_bytes()[0])
    }
    fn substr(&self, start: usize, end: usize) -> &OsStr
    {
        unsafe { OsStr::from_encoded_bytes_unchecked(&self.as_encoded_bytes()[start..end]) }
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    mod starts_with
    {
        use super::*;

        #[test]
        fn starts_with()
        {
            let haystack = OsStr::new("/path/to/file.txt");
            let needle = OsStr::new("/path/to");
            assert!(haystack.starts_with(needle));
        }

        #[test]
        fn does_not_start_with()
        {
            let haystack = OsStr::new("/path/to/file.txt");
            let needle = OsStr::new("/root");
            assert!(!haystack.starts_with(needle));
        }

        #[test]
        fn empty_needle()
        {
            let haystack = OsStr::new("/path/to");
            let needle = OsStr::new("");
            assert!(haystack.starts_with(needle));
        }

        #[test]
        fn needle_longer_than_haystack()
        {
            let haystack = OsStr::new("hi");
            let needle = OsStr::new("hello");
            assert!(!haystack.starts_with(needle));
        }
    }
    #[test]
    fn ends_with_true() {
        let haystack = OsStr::new("/path/to/file.txt");
        let needle = OsStr::new(".txt");
        assert!(haystack.ends_with(needle));
    }

    #[test]
    fn ends_with_false() {
        let haystack = OsStr::new("/path/to/file.txt");
        let needle = OsStr::new(".md");
        assert!(!haystack.ends_with(needle));
    }

    #[test]
    fn ends_with_empty_needle() {
        let haystack = OsStr::new("/path/to");
        let needle = OsStr::new("");
        assert!(haystack.ends_with(needle));
    }

    #[test]
    fn ends_with_needle_longer_than_haystack() {
        let haystack = OsStr::new("hi");
        let needle = OsStr::new("hello");
        assert!(!haystack.ends_with(needle));
    }

    #[test]
    fn equal_strings() {
        let haystack = OsStr::new("test");
        let needle = OsStr::new("test");
        assert!(haystack.starts_with(needle));
        assert!(haystack.ends_with(needle));
    }
}