use std::collections::HashMap;
use std::error::Error;
use crate::block_meta::{BlockDesignMeta, BlockRuntimeMeta};
use crate::blocks::PlugList;
use crate::vars::ScopeChanges;
use crate::{BlockId, ImpulseActions, ImpulseBlock, InstRunId, LatchActions, LatchBlock, LocalScope, Runtime, Scope, SharedScope};
use nab_3l14::Signal;
use std::fmt::Debug;
use bitcode::{Decode, Encode};
use triomphe::Arc;
use asset_3l14::{AssetLifecycler, AssetLoadRequest};
use debug_3l14::debug_gui::DebugGui;

#[proc_macros_3l14::asset]
#[derive(Debug)]
pub struct Circuit
{
    pub auto_entries: EntryPoints,
    pub signaled_entries: Box<[(Signal, EntryPoints)]>,
    pub impulses: Box<[Box<dyn ImpulseBlock>]>,
    pub latches: Box<[Box<dyn LatchBlock>]>,
    pub num_local_vars: u32,
}

#[derive(Encode, Decode)]
pub struct CircuitFile
{
    pub auto_entries: EntryPoints,
    pub signaled_entries: Box<[(Signal, EntryPoints)]>,
    pub impulses: Box<[(u64, u64)]>,
    pub latches: Box<[(u64, u64)]>,
    pub num_local_vars: u32,
}

#[derive(Encode, Decode)]
pub struct BlockDebugData<'b>
{
    name: &'b str,
}

#[derive(Encode, Decode)]
pub struct CircuitDebugData<'d>
{
    block_names: &'d[&'d str]
}

pub type EntryPoints = Box<[BlockId]>;

struct CircuitLifecycler
{
    known_impulses: HashMap<u64, &'static BlockRuntimeMeta<0>>,
    known_latches: HashMap<u64, &'static BlockRuntimeMeta<1>>,
}
impl Default for CircuitLifecycler
{
    fn default() -> Self
    {
        let known_impulses = inventory::iter::<BlockRuntimeMeta<0>>()
            .map(|b| (b.type_name_hash, b)).collect();
        let known_latches = inventory::iter::<BlockRuntimeMeta<1>>()
            .map(|b| (b.type_name_hash, b)).collect();
        Self { known_impulses, known_latches }
    }
}
impl AssetLifecycler for CircuitLifecycler
{
    type Asset = Circuit;

    fn load(&self, request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        todo!()
    }
}

// A helper struct that contains all the types required to test circuit blocks
pub struct TestContext
{
    pub local_scope: LocalScope,
    pub local_changes: ScopeChanges,
    pub shared_scope: SharedScope,
    pub shared_changes: ScopeChanges,

    pub pulse_outlets: PlugList,
    pub latch_outlets: PlugList,
    pub runtime: Arc<Runtime>,
}
impl Default for TestContext
{
    fn default() -> Self
    {
        Self
        {
            local_scope: LocalScope::new(2), // todo: re-eval?
            local_changes: Default::default(),
            shared_scope: Default::default(),
            shared_changes: Default::default(),

            pulse_outlets: Default::default(),
            latch_outlets: Default::default(),
            runtime: Runtime::new(),
        }
    }
}
impl TestContext
{
    pub fn pulse(&mut self, impulse: impl ImpulseBlock)
    {
        let mut _rs = None;
        impulse.pulse(Scope
        {
            run_id: InstRunId::TEST,
            block_id: BlockId::impulse(0),
            local_scope: &mut self.local_scope,
            local_changes: &mut self.local_changes,
            shared_scope: &self.shared_scope,
            shared_changes: &mut self.shared_changes,
            latch_context: &mut _rs,
        }, ImpulseActions
        {
            pulse_outlets: &mut self.pulse_outlets,
            runtime: self.runtime.clone(),
        });
    }

    pub fn latch(&mut self, latch: impl LatchBlock)
    {
        let mut _rs = None; // store?
        latch.power_on(Scope
        {
            run_id: InstRunId::TEST,
            block_id: BlockId::latch(0),
            local_scope: &mut self.local_scope,
            local_changes: &mut self.local_changes,
            shared_scope: &self.shared_scope,
            shared_changes: &mut self.shared_changes,
            latch_context: &mut _rs,
        }, LatchActions
        {
            pulse_plugs: &mut self.pulse_outlets,
            latch_plugs: &mut self.latch_outlets,
            runtime: self.runtime.clone(),
        });
    }

    pub fn unloatch(&mut self, latch: impl LatchBlock)
    {
        let mut _rs = None; // store?
        latch.power_off(Scope
        {
            run_id: InstRunId::TEST,
            block_id: BlockId::latch(0),
            local_scope: &mut self.local_scope,
            local_changes: &mut self.local_changes,
            shared_scope: &self.shared_scope,
            shared_changes: &mut self.shared_changes,
            latch_context: &mut _rs,
        });
    }

    // todo: var change, re-enter
}

#[cfg(test)]
mod tests
{
    use std::fmt::Formatter;
    use crate::BlockKind;
    use super::*;

    #[test]
    fn block_id()
    {
        let block = BlockId::impulse(0);
        assert!(matches!(block.kind(), BlockKind::Impulse));;

        let block = BlockId::latch(0);
        assert!(matches!(block.kind(), BlockKind::Latch));;
    }

    #[test]
    fn yolo()
    {
        mod foo
        {
            use std::alloc::Layout;
            use std::fmt::{Debug, Formatter};
            use std::slice;
            use triomphe::Arc;
            use bitcode::Decode;

            pub struct Mule<T: Sized>
            {
                value: T,
                buffer: [u8],
            }
            impl<T: Debug> Debug for Mule<T>
            {
                fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { T::fmt(&self.value, f) }
            }
            impl<T> std::ops::Deref for Mule<T>
            {
                type Target = T;

                fn deref(&self) -> &Self::Target
                {
                    &self.value
                }
            }
            impl<'d, T: Decode<'d> + Sized> Mule<T>
            {
                pub fn new_arc(encoded: &'d [u8]) -> Arc<Mule<T>>
                {
                    let ns = size_of::<T>();
                    unsafe
                    {
                        let layout = Layout::from_size_align_unchecked(ns + encoded.len(), align_of::<T>());
                        let mut ptr = std::alloc::alloc(layout);
                        std::ptr::copy_nonoverlapping(encoded.as_ptr().byte_add(ns), ptr, encoded.len());

                        let mut mule = &mut *(ptr as *mut T);
                        *mule = bitcode::decode(encoded).unwrap();
                        // gross
                        let fat = slice::from_raw_parts(mule, encoded.len()) as *const _ as *const [u8];
                        Arc::from_raw(fat as *mut Self)
                    }
                }

                pub fn new_box(encoded: &'d [u8]) -> Box<Mule<T>>
                {
                    let ns = size_of::<T>();
                    unsafe
                    {
                        let layout = std::alloc::Layout::from_size_align_unchecked(ns + encoded.len(), align_of::<T>());
                        let mut ptr = std::alloc::alloc(layout);
                        std::ptr::copy_nonoverlapping(encoded.as_ptr().byte_add(ns), ptr, encoded.len());

                        let mut mule = &mut *(ptr as *mut T);
                        *mule = bitcode::decode(encoded).unwrap();
                        // gross
                        let fat = slice::from_raw_parts(mule, encoded.len()) as *const _ as *const [u8];
                        Box::from_raw(fat as *mut Self)
                    }
                }
            }
        }
        use foo::*;

        #[derive(bitcode::Encode, bitcode::Decode, Debug)]
        struct Names<'a>
        {
            pub names: [&'a str; 3],
        }

        let n = Names { names: ["Test 1", "Asdf asdf 2", "Bompu quelch 3"] };

        let enc = bitcode::encode(&n);
        let z = Mule::<Names>::new_arc(&enc);

        println!("<<< {n:?}\n");
        println!(">>> {z:?}\n");
        println!("!!! {:?}\n", z.names);
    }
}
