use std::any::{TypeId, Any};
use std::collections::HashMap;

// unify this with middlewares? (May require different underlying container types in the future)

type GlobalStorage = HashMap<TypeId, Box<dyn Any>>;

#[derive(Debug, Default)]
pub struct Globals
{
    storage: GlobalStorage,
}
impl<'a> Globals
{
    pub fn try_init<TGlobal: Default + 'static>(&mut self) -> Result<&mut TGlobal, super::Errors::AlreadyExists>
    {
        let gty = TypeId::of::<TGlobal>();
        if self.storage.contains_key(&gty)
        {
            return Err(super::Errors::AlreadyExists{});
        }

        let boxed: Box<dyn Any> = Box::<TGlobal>::new(Default::default());
        self.storage.insert(gty, boxed); // todo: try_insert
        Ok(self.get_mut::<TGlobal>().unwrap())
    }
    pub fn try_add<TGlobal: 'static>(&mut self, global: TGlobal) -> Result<(), super::Errors::AlreadyExists>
    {
        let gty: TypeId = TypeId::of::<TGlobal>();
        if self.storage.contains_key(&gty)
        {
            return Err(super::Errors::AlreadyExists{});
        }

        self.storage.insert(gty, Box::new(global));
        Ok(())
    }

    pub fn remove<TGlobal: 'static>(&mut self) -> Option<Box<TGlobal>>
    {
        let gty: TypeId = TypeId::of::<TGlobal>();
        self.storage.remove(&gty).map(|g| unsafe { Box::from_raw(Box::into_raw(g) as *mut TGlobal) }) // downcast_unchecked?
    }

    pub fn get<TGlobal: 'static>(&self) -> Option<&TGlobal>
    {
        let gty = TypeId::of::<TGlobal>();
        match self.storage.get(&gty)
        {
            Some(global) => Some(global.downcast_ref().unwrap()), // TODO: unchecked
            None => None,
        }
    }
    pub fn get_mut<TGlobal: 'static>(&mut self) -> Option<&mut TGlobal>
    {
        let gty = TypeId::of::<TGlobal>();
        match self.storage.get_mut(&gty)
        {
            Some(global) => Some(global.downcast_mut().unwrap()), // TODO: unchecked
            None => None,
        }
    }
    pub fn contains_type<TGlobal: 'static>(&self) -> bool { self.storage.contains_key(&TypeId::of::<TGlobal>()) }

    pub fn len(&self) -> usize { self.storage.len() }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[derive(Default)]
    struct TestGlobal(bool);

    #[test]
    fn test_globals()
    {
        let mut globals: Globals = Default::default();
        assert_eq!(0, globals.len());
        assert!(!globals.contains_type::<TestGlobal>());

        let test_g = TestGlobal(false);
        assert!(globals.try_add(test_g).is_ok());
        assert_eq!(1, globals.len());
        assert!(globals.contains_type::<TestGlobal>());
        assert!(globals.try_init::<TestGlobal>().is_err());

        let _ = globals.get::<TestGlobal>().unwrap();
        let gotten = globals.get_mut::<TestGlobal>().unwrap();
        assert_eq!(false, gotten.0);
        gotten.0 = true;
        assert_eq!(1, globals.len());
        assert!(globals.contains_type::<TestGlobal>());

        assert!(globals.try_add(TestGlobal(true)).is_err());
        let removed = globals.remove::<TestGlobal>().unwrap();
        assert_eq!(true, removed.as_ref().0);
        assert_eq!(0, globals.len());
        assert!(!globals.contains_type::<TestGlobal>());

        assert!(globals.try_init::<TestGlobal>().is_ok());
        assert_eq!(1, globals.len());
        assert_eq!(false, globals.get::<TestGlobal>().unwrap().0);
    }
}