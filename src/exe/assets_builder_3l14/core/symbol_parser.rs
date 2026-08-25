use std::collections::HashMap;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use dashmap::DashMap;
use indexmap::IndexMap;
use metrohash::MetroHash64;
use walkdir::WalkDir;
use nab_3l14::utils::hash_bstrings;
use nab_3l14::Symbol;

// Look up symbols by name and map them to their ID
pub struct SymbolsDict
{
    symbols_file_root: PathBuf,
    lookup: DashMap<u32, HashMap<String, u32>>,
}
impl SymbolsDict
{
    pub fn new(symbols_file_root: PathBuf) -> Self
    {
        Self
        {
            symbols_file_root,
            lookup: DashMap::new(),
        }
    }

    pub fn get<S: Symbol>(&self, symbol_name: &str) -> Option<S>
    {
        let type_hash = S::TYPE_NAME_HASH;
        let entry = self.lookup.entry(type_hash).or_try_insert_with(||
            {
                let symbol_file = self.symbols_file_root.join(Path::new(S::TYPE_NAME));
                let parsed: HashMap<String, u32> = toml::from_slice(&std::fs::read(symbol_file).map_err(|_| ())?).map_err(|_| ())?;
                Ok::<_, ()>(parsed)
            }).ok()?;
        entry.get(symbol_name).map(|id| S::from_raw(*id))
    }
}

type SymbolEntries<'s> = IndexMap<&'s str, u32>;

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

pub fn validate_symbols(root_dir: impl AsRef<Path>) -> bool
{
    let validator_version_hash: u64 = hash_bstrings(0, &[
        b"Initial",
        b"Moved to using iterators"
    ]);

    let walker = WalkDir::new(root_dir.as_ref());
    let mut success = true;
    for maybe_dir in walker
    {
        match maybe_dir
        {
            Ok(dent) =>
            {
                if !dent.file_type().is_file() { continue; }

                // todo: validate file name against known symbol types

                let Ok(bytes) = std::fs::read(dent.path()) else { continue; }; // failure?
                let Ok(entries) = toml::from_slice(&bytes) else
                {
                    log::error!("Failed to parse symbols file {:?} as TOML", dent.file_name());
                    success = false;
                    continue;
                };

                match validate(validator_version_hash, entries)
                {
                    SymbolValidation::Success { hash } =>
                    {
                        log::info!("Validated {:?} [version={:#016x}]", dent.file_name(), hash); // log debug?
                    }
                    SymbolValidation::Error(err) =>
                    {
                        log::error!("Validation failed for {:?}: {:?}", dent.file_name(), err);
                        success = false;
                    }
                }
            }
            Err(err) =>
            {
                log::error!("Failed to traverse tables {:?}", err);
                success = false;
            }
        }
    }
    success
}

#[must_use]
fn validate(hash_seed: u64, table: SymbolEntries) -> SymbolValidation
{
    let mut hasher = MetroHash64::with_seed(hash_seed);
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
    use indexmap::IndexMap;
    use super::*;

    // todo: test specific errors

    #[test]
    fn good()
    {
        let zoops = IndexMap::from([
            ("test1", 1),
            ("test2", 2),
            ("test3", 10),
            ("test4", 11),
            ("test5", 100),
        ]);
        match validate(0, zoops)
        {
            SymbolValidation::Success { .. } => {}
            SymbolValidation::Error(err) => panic!("expected validation success, but got {err:?}"),
        }
    }

    #[test]
    fn bad_dupe_ids()
    {
        let zoops = IndexMap::from([
            ("test1", 1),
            ("test2", 1),
        ]);
        let SymbolValidation::Error(..) = validate(0, zoops) else { panic!("Expected validation error!"); };
    }
    #[test]
    fn bad_id_order()
    {
        let zoops = IndexMap::from([
            ("test1", 10),
            ("test2", 1),
            ("test3", 10),
        ]);
        let SymbolValidation::Error(..) = validate(0, zoops) else { panic!("Expected validation error!"); };
    }
    #[test]
    fn bad_zero()
    {
        let zoops = IndexMap::from([
            ("test0", 0),
        ]);
        let SymbolValidation::Error(..) = validate(0, zoops) else { panic!("Expected validation error!"); };
    }
}