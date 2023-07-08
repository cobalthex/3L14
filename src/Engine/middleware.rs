use std::any::TypeId;
use std::{vec};

use super::app::AppContext;
use super::CompletionState;

/// Middlewares can interact with the app but are otherwise isolated. These are what drive the app
/// Any data that needs to be shared should be stored in globals
/// Only private data to the middleware should be stored with the middleware
/// Middlewares are stored/evaluated in order of insertion
pub trait Middleware
{
    fn name(&self) -> &str; // a canonical name for this middleware

    // async startup/shutdown?

    fn startup(&mut self, app: &mut AppContext) -> CompletionState; // Initialize this middleware, this is called every tick and should return Completed once ready
    fn shutdown(&mut self, app: &mut AppContext) -> CompletionState; // Uninitialize this middleware, this is called every tick and should return Completed once torn down
    fn run(&mut self, app: &mut AppContext) -> CompletionState; // Run this middleware, this is called every tick and should return Completed when the app should shutdown (any middleware completing will cause the app to shut down)
}

type MiddlewareStorage = Vec::<(TypeId, Box<dyn Middleware>)>; // store in a vec?

#[derive(Default)]
pub struct Middlewares
{
    storage: MiddlewareStorage,
}
impl Middlewares
{
    fn index_of(&self, type_id: TypeId) -> Option<usize>
    {
        for i in 0..self.storage.len()
        {
            if self.storage[i].0 == type_id
            {
                return Some(i)
            }
        }
        None
    }

    pub fn try_add<TMiddleware: Middleware + 'static>(&mut self, middleware: TMiddleware) -> Result<(), super::Errors::AlreadyExists>
    {
        let mty: TypeId = TypeId::of::<TMiddleware>();
        match self.index_of(mty)
        {
            Some(_) => Err(super::Errors::AlreadyExists{}),
            None =>
            {
                self.storage.push((mty, Box::new(middleware)));
                Ok(())
            }
        }
    }

    pub fn remove<TMiddleware: Middleware + 'static>(&mut self) -> Option<Box<TMiddleware>>
    {
        let mty: TypeId = TypeId::of::<TMiddleware>();
        self.index_of(mty)
            .and_then(|i| Some(unsafe { Box::from_raw(Box::into_raw(self.storage.remove(i).1) as *mut TMiddleware) }))
    }

    pub fn get<TMiddleware: Middleware + 'static>(&self) -> Option<&TMiddleware>
    {
        let mty = TypeId::of::<TMiddleware>();
        match self.index_of(mty)
        {
            Some(i) => Some(unsafe { &*(self.storage[i].1.as_ref() as *const dyn Middleware as *const TMiddleware) }), // yum
            None => None,
        }
    }
    pub fn get_mut<TMiddleware: Middleware + 'static>(&mut self) -> Option<&mut TMiddleware>
    {
        let mty = TypeId::of::<TMiddleware>();
        match self.index_of(mty)
        {
            Some(i) => Some(unsafe { &mut *(self.storage[i].1.as_mut() as *mut dyn Middleware as *mut TMiddleware) }), // yum
            None => None,
        }
    }
    pub fn contains_type<TMiddleware: 'static>(&self) -> bool { self.index_of(TypeId::of::<TMiddleware>()).is_some() }

    pub fn iter(&self) -> std::slice::Iter<(TypeId, Box<dyn Middleware>)>
    {
        self.storage.iter()
    }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<(TypeId, Box<dyn Middleware>)>
    {
        self.storage.iter_mut()
    }
    pub fn len(&self) -> usize { self.storage.len() }
}
impl core::fmt::Debug for Middlewares
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("{todo Middlewares}")
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    struct TestMiddleware(pub bool);
    impl Middleware for TestMiddleware
    {
        fn name(&self) -> &str { "TestMiddleware" }
        fn startup(&mut self, _app: &mut AppContext) -> CompletionState { CompletionState::Completed }
        fn shutdown(&mut self, _app: &mut AppContext) -> CompletionState { CompletionState::Completed }
        fn run(&mut self, _app: &mut AppContext) -> CompletionState { CompletionState::Completed }
    }

    #[test]
    fn test_middlewares()
    {
        let mut middlewares: Middlewares = Default::default();
        assert_eq!(0, middlewares.len());
        assert_eq!(0, middlewares.iter().count());
        assert!(!middlewares.contains_type::<TestMiddleware>());

        let test_mware = TestMiddleware(false);
        assert!(middlewares.try_add(test_mware).is_ok());
        assert_eq!(1, middlewares.len());
        assert!(middlewares.contains_type::<TestMiddleware>());
        let first = middlewares.iter().next().unwrap();
        assert_eq!(TypeId::of::<TestMiddleware>(), first.0);
        assert_eq!("TestMiddleware", first.1.name());

        let _ = middlewares.get::<TestMiddleware>().unwrap();
        let gotten = middlewares.get_mut::<TestMiddleware>().unwrap();
        assert_eq!(false, gotten.0);
        assert_eq!("TestMiddleware", gotten.name());
        gotten.0 = true;
        assert_eq!(1, middlewares.len());
        assert!(middlewares.contains_type::<TestMiddleware>());

        assert!(middlewares.try_add(TestMiddleware(true)).is_err());
        let removed = middlewares.remove::<TestMiddleware>().unwrap();
        assert_eq!(0, middlewares.len());
        assert!(!middlewares.contains_type::<TestMiddleware>());
        assert_eq!(true, removed.as_ref().0);
    }
}