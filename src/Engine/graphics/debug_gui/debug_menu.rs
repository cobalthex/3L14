use std::hash::{Hash, Hasher};
use egui::Pos2;
use indexmap::IndexMap;
use crate::engine::graphics::debug_gui::DebugGuiBase;

pub struct DebugMenuId(u64);

struct DebugMenuItemState
{
    name: String,
    is_active: bool,
}

#[derive(Default)]
pub struct DebugMenuMemory
{
    pub is_active: bool,
    states: IndexMap<u64, DebugMenuItemState>, // todo: this should be sorted
}
impl DebugMenuMemory
{
    pub fn gui_id_by_name<'m, T: DebugGuiBase<'m>>(name: &str) -> DebugMenuId
    {
        let mut name_hasher = std::hash::DefaultHasher::new();
        name.hash(&mut name_hasher);
        // TypeId::of::<T>().hash(&mut name_hasher);
        DebugMenuId(name_hasher.finish())
    }

    #[inline]
    pub fn gui_id<'m, T: DebugGuiBase<'m>>(gui: &'m T) -> DebugMenuId
    {
        Self::gui_id_by_name::<T>(gui.name())
    }

    fn get_or_create_state<'m, T: DebugGuiBase<'m>>(&mut self, gui: &'m T) -> &mut DebugMenuItemState
    {
        let gui_id = Self::gui_id(gui);
        self.states.entry(gui_id.0).or_insert_with(||
        {
            DebugMenuItemState
            {
                name: gui.name().to_string(),
                is_active: false
            }
        })
    }

    // set a state in the memory, will add if it doesn't exist. Returns the new state
    pub fn set_active<'m, T: DebugGuiBase<'m>>(&mut self, gui: &'m T, activate: bool)
    {
        self.get_or_create_state(gui).is_active = activate;
    }

    pub fn set_active_by_name<'m, T: DebugGuiBase<'m>>(&mut self, name: &str, activate: bool)
    {
        let gui_id = Self::gui_id_by_name::<T>(name);
        self.states.entry(gui_id.0).or_insert_with(||
            {
                DebugMenuItemState
                {
                    name: name.to_string(),
                    is_active: false
                }
            }).is_active = activate;
    }

    // toggle a state in the memory, does nothing if the state doesn't already exist. Returns the new state
    pub fn toggle_active<'m, T: DebugGuiBase<'m>>(&mut self, gui: &'m T) -> Option<bool>
    {
        let gui_id = Self::gui_id(gui);
        match self.states.get_mut(&gui_id.0)
        {
            None => None,
            Some(state) =>
            {
                state.is_active ^= true;
                Some(state.is_active)
            }
        }
    }
}

pub struct DebugMenu<'m>
{
    memory: &'m mut DebugMenuMemory, // todo: mutexed
    debug_gui: egui::Context,
}
impl<'m> DebugMenu<'m>
{
    pub fn new(memory: &'m mut DebugMenuMemory, debug_gui: &'m egui::Context) -> Self
    {
        Self
        {
            memory,
            debug_gui: debug_gui.clone(),
        }
    }

    // todo: categories

    pub fn present(&mut self)
    {
        egui::Window::new("Debug GUI Menu")
            .movable(true)
            .title_bar(false)
            .default_pos(Pos2 { x: 20.0, y: 20.0 })
            .open(&mut self.memory.is_active)
            .show(&self.debug_gui, |ui|
                {
                    for gui in self.memory.states.values_mut()
                    {
                        ui.checkbox(&mut gui.is_active, &gui.name);
                    }
                });
    }

    // todo: accept slice? (might need some hlist magic)
    pub fn add<T: DebugGuiBase<'m>>(&mut self, gui: &'m T)
    {
        // todo: sorted dict

        let state = self.memory.get_or_create_state(gui);
        gui.debug_gui_base(&mut state.is_active, &self.debug_gui)
    }
}