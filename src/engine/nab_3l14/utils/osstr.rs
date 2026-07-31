use std::ffi::OsStr;

pub trait OsStrUtils
{
    fn starts_with(&self, needle: &OsStr) -> bool;
    fn ends_with(&self, needle: &OsStr) -> bool;
}
impl OsStrUtils for OsStr
{
    fn starts_with(&self, needle: &OsStr) -> bool
    {
        self.as_encoded_bytes().starts_with(needle.as_encoded_bytes())
    }
    fn ends_with(&self, needle: &OsStr) -> bool
    {
        self.as_encoded_bytes().ends_with(needle.as_encoded_bytes())
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    
    #[test]
    fn starts_With_true() {
        let haystack = OsStr::new("/path/to/file.txt");
        let needle = OsStr::new("/path/to");
        assert!(haystack.starts_with(needle));
    }

    #[test]
    fn starts_With_false() {
        let haystack = OsStr::new("/path/to/file.txt");
        let needle = OsStr::new("/root");
        assert!(!haystack.starts_with(needle));
    }

    #[test]
    fn starts_With_empty_needle() {
        let haystack = OsStr::new("/path/to");
        let needle = OsStr::new("");
        assert!(haystack.starts_with(needle));
    }

    #[test]
    fn starts_With_needle_longer_than_haystack() {
        let haystack = OsStr::new("hi");
        let needle = OsStr::new("hello");
        assert!(!haystack.starts_with(needle));
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