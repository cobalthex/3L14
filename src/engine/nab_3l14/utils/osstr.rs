use std::{ffi::OsStr, ops::{Bound, RangeBounds}};

pub trait OsStrUtils
{
    fn starts_with(&self, needle: impl AsRef<OsStr>) -> bool;
    fn ends_with(&self, needle: impl AsRef<OsStr>) -> bool;

    fn strip_prefix(&self, prefix: impl AsRef<OsStr>) -> Option<&OsStr>;
    fn strip_suffix(&self, suffix: impl AsRef<OsStr>) -> Option<&OsStr>;

    fn position(&self, predicate: impl AsRef<OsStr>) -> Option<usize>;

    // Get a substring of this `OsStr` by index range (of bytes)
    // SAFETY:
    // The caller is responsible for only slicing at valid encoded
    // boundaries, matching the contract of `slice_encoded_bytes`.
    fn substr(&self, range: impl RangeBounds<usize>) -> &OsStr;
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
    fn strip_prefix(&self, prefix: impl AsRef<OsStr>) -> Option<&OsStr>
    {
        let bytes = self.as_encoded_bytes();
        let prefix_bytes = prefix.as_ref().as_encoded_bytes();
        bytes.strip_prefix(prefix_bytes).map(|stripped| unsafe { OsStr::from_encoded_bytes_unchecked(stripped) })
    }
    fn strip_suffix(&self, suffix: impl AsRef<OsStr>) -> Option<&OsStr>
    {
        let bytes = self.as_encoded_bytes();
        let suffix_bytes = suffix.as_ref().as_encoded_bytes();
        bytes.strip_suffix(suffix_bytes).map(|stripped| unsafe { OsStr::from_encoded_bytes_unchecked(stripped) })
    }
    fn position(&self, predicate: impl AsRef<OsStr>) -> Option<usize>
    {
        let bytes = self.as_encoded_bytes();
        let predicate_bytes = predicate.as_ref().as_encoded_bytes();
        if predicate_bytes.len() == 0 { return Some(0); }
        bytes.windows(predicate_bytes.len()).position(|test| test == predicate_bytes)
    }
    fn substr(&self, range: impl RangeBounds<usize>) -> &OsStr
    {
        // todo: use OsStr::slice_encoded_bytes once it has stabilized
        let bytes = self.as_encoded_bytes();

        let start = match range.start_bound()
        {
            Bound::Included(&i) => i,
            Bound::Excluded(&i) => i.checked_add(1).expect("range start overflow"),
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound()
        {
            Bound::Included(&i) => i.checked_add(1).expect("range end overflow"),
            Bound::Excluded(&i) => i,
            Bound::Unbounded => bytes.len(),
        };

        // Preserve the panic behavior of normal slice indexing.
        let bytes = &bytes[start..end];
        unsafe { OsStr::from_encoded_bytes_unchecked(bytes) }
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn equal_strings()
    {
        let haystack = OsStr::new("test");
        let needle = OsStr::new("test");
        assert!(haystack.starts_with(needle));
        assert!(haystack.ends_with(needle));
    }

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

    mod ends_with
    {
        use super::*;

        #[test]
        fn ends_with()
        {
            let haystack = OsStr::new("/path/to/file.txt");
            let needle = OsStr::new(".txt");
            assert!(haystack.ends_with(needle));
        }

        #[test]
        fn does_not_end_with()
        {
            let haystack = OsStr::new("/path/to/file.txt");
            let needle = OsStr::new(".md");
            assert!(!haystack.ends_with(needle));
        }

        #[test]
        fn empty_needle()
        {
            let haystack = OsStr::new("/path/to");
            let needle = OsStr::new("");
            assert!(haystack.ends_with(needle));
        }

        #[test]
        fn needle_longer_than_haystack()
        {
            let haystack = OsStr::new("hi");
            let needle = OsStr::new("hello");
            assert!(!haystack.ends_with(needle));
        }
    }

    mod position
    {
        use super::*;

        #[test]
        fn position()
        {
            let haystack = OsStr::new("/path/to/file.txt");
            let needle = OsStr::new("to");
            assert_eq!(haystack.position(needle), Some(6));
        }

        #[test]
        fn not_found()
        {
            let haystack = OsStr::new("/path/to/file.txt");
            let needle = OsStr::new("not_found");
            assert_eq!(haystack.position(needle), None);
        }

        #[test]
        fn empty_haystack()
        {
            let haystack = OsStr::new("");
            let needle = OsStr::new("test");
            assert_eq!(haystack.position(needle), None);
        }

        #[test]
        fn empty_needle()
        {
            let haystack = OsStr::new("/path/to/file.txt");
            let needle = OsStr::new("");
            assert_eq!(haystack.position(needle), Some(0));
        }
    }

    // TODO: prefix + suffix tests

    mod substr
    {
        use super::*;

        #[test]
        fn substr()
        {
            let haystack = OsStr::new("/path/to/file.txt");
            assert_eq!(haystack.substr(6..), OsStr::new("to/file.txt"));
        }

        #[test]
        fn empty_range()
        {
            let haystack = OsStr::new("/path/to/file.txt");
            assert_eq!(haystack.substr(..), OsStr::new("/path/to/file.txt"));
        }

        // is full test suite necessary here?
    }
}
