use std::collections::HashSet;
use std::fs::File;
use std::hash::{DefaultHasher, Hasher};
use std::io::{Error, Read};
use std::path::{Path, PathBuf};
use std::ptr::hash;
use log::error;
use metrohash::MetroHash64;
use serde::Deserialize;
use walkdir::WalkDir;
use nab_3l14::utils::hash_bstrings;
use regex::{Regex, RegexBuilder};

#[derive(Debug)]
pub enum SymbolValidationError
{
    InvalidID((String, u32)),
    DuplicateName((String, u32)),
}

#[derive(Debug)]
pub enum SymbolValidation
{
    Success
    {
        hash: u64,
    },
    Error(Box<[SymbolValidationError]>),
}

type SymbolEntries<'s> = Box<[(&'s str, u32)]>;

pub fn validate_syhmbols(root_dir: impl AsRef<Path>) -> bool
{
    let validator_version_hash: u64 = hash_bstrings(0, &[
        b"Initial"
    ]);

    let regex = RegexBuilder::new(r"([a-zA-Z_]\w+)\s*=\s*(\d+)")
        .build()
        .unwrap();

    let mut walker = WalkDir::new(root_dir.as_ref());
    let mut success = true;
    for maybe_dir in walker
    {
        match maybe_dir
        {
            Ok(dir) =>
            {
                if !dir.file_type().is_file() { continue; }

                // todo: validate file name against known symbol types

                fn read_to_string(p: &Path) -> Result<String, std::io::Error>
                {
                    let mut s = String::new();
                    File::open(p)?.read_to_string(&mut s)?;
                    Ok(s)
                }

                let text = match read_to_string(dir.path())
                {
                    Ok(str) => str,
                    Err(err) =>
                    {
                        log::error!("Failed to read {:?}: {:?}", dir.file_name(), err);
                        success = false;
                        continue;
                    }
                };

                let mut entries = Vec::new();
                for line in text.lines()
                {
                    let Some(regmatch) = regex.captures(line) else
                    {
                        log::error!("Failed to parse line {:?}", line);
                        continue;
                    };
                    let name = regmatch.get(1).unwrap().as_str();
                    let Ok(id) = regmatch.get(2).unwrap().as_str().parse::<u32>() else
                    {
                        log::error!("Failed to parse ID from {:?}", line);
                        continue;
                    };
                    entries.push((name, id));
                }

                match validate(validator_version_hash, entries.into_boxed_slice())
                {
                    SymbolValidation::Success { hash } =>
                    {
                        log::info!("Validated {:?} [version={:#016x}]", dir.file_name(), hash); // log debug?
                    }
                    SymbolValidation::Error(err) =>
                    {
                        log::error!("Validation failed for {:?}: {:?}", dir.file_name(), err);
                        success = false;
                    }
                }
            }
            Err(err) =>
            {
                log::error!("Failed to traverse tables {:?}", err);

            }
        }
    }
    success
}

#[must_use]
fn validate(hash_seed: u64, table: SymbolEntries) -> SymbolValidation
{
    let mut hasher = MetroHash64::with_seed(hash_seed);
    let mut seen = HashSet::new();
    let mut errors = Vec::new();
    let mut max = 0u32; // values must start at 1
    for (name, id) in table
    {
        if id <= max
        {
            errors.push(SymbolValidationError::InvalidID((name.to_string(), id)));
        }

        hasher.write(name.as_bytes());
        hasher.write_u32(id);

        // TODO: validate unique names
        if !seen.insert(name)
        {
            errors.push(SymbolValidationError::DuplicateName((name.to_string(), id)));
        }

        max = id;
    }


    match errors.is_empty()
    {
        true => SymbolValidation::Success { hash: hasher.finish() },
        false => SymbolValidation::Error(errors.into_boxed_slice()),
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    // todo: test specific errors

    #[test]
    fn good()
    {
        let zoops = Box::new([
            ("test1".into(), 1),
            ("test2".into(), 2),
            ("test3".into(), 10),
            ("test4".into(), 11),
            ("test5".into(), 100),
        ]);
        let SymbolValidation::Success { hash } = validate(0, zoops) else { panic!("failed"); };
    }

    #[test]
    fn bad_dupe_ids()
    {
        let zoops = Box::new([
            ("test1".into(), 1),
            ("test2".into(), 1),
        ]);
        let SymbolValidation::Error(..) = validate(0, zoops) else { panic!("failed"); };
    }
    #[test]
    fn bad_dup_names()
    {
        let zoops = Box::new([
            ("test".into(), 1),
            ("test".into(), 10),
        ]);
        let SymbolValidation::Error(..) = validate(0, zoops) else { panic!("failed"); };
    }
    #[test]
    fn bad_id_order()
    {
        let zoops = Box::new([
            ("test1".into(), 10),
            ("test2".into(), 1),
            ("test3".into(), 10),
        ]);
        let SymbolValidation::Error(..) = validate(0, zoops) else { panic!("failed"); };
    }
    #[test]
    fn bad_zero()
    {
        let zoops = Box::new([
            ("test0".into(), 0),
        ]);
        let SymbolValidation::Error(..) = validate(0, zoops) else { panic!("failed"); };
    }
}