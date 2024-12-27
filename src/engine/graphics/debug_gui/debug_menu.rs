use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::Path;
use egui::Pos2;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use crate::engine::graphics::debug_gui::DebugGuiBase;

pub struct DebugMenuId(u64);

#[derive(Serialize, Deserialize)]
struct DebugMenuItemState
{
    name: String,
    is_active: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct DebugMenuMemory
{
    #[serde(skip)]
    generation: usize, // updated everytime a state changes
    #[serde(skip)]
    last_saved_generation: usize,

    is_active: bool, // is the top level menu active
    states: IndexMap<u64, DebugMenuItemState>, // todo: this should be sorted
}
impl DebugMenuMemory
{
    pub fn generation(&self) -> usize { self.generation }

    // returns true if wrote, false if not dirty
    // will always update dirty state, even on failure
    pub fn save_if_dirty(&mut self, path: impl AsRef<Path>) -> bool
    {
        if self.generation == self.last_saved_generation { return false; }
        self.last_saved_generation = self.generation;

        let toml = match toml::to_string(&self)
        {
            Ok(toml) => toml,
            Err(e) =>
            {
                log::warn!("Failed to serialize debug GUI state: {e}");
                return false;
            },
        };
        let mut fwrite = match std::fs::File::create(&path)
        {
            Ok(file) => file,
            Err(e) =>
            {
                log::warn!("Failed to open debug GUI state file '{:?}' for writing: {e}", path.as_ref());
                return false;
            },
        };
        if let Err(e) = fwrite.write(toml.as_bytes())
        {
            log::warn!("Failed to write debug GUI state to file: {e}");
            return false;
        };
        true
    }

    pub fn set_active(&mut self, is_active: bool)
    {
        if self.is_active != is_active
        {
            self.is_active = is_active;
            self.generation += 1;
        }
    }

    pub fn toggle_active(&mut self)
    {
        self.is_active ^= true;
        self.generation += 1;
    }

    pub fn gui_id_by_name<T: DebugGuiBase>(name: &str) -> DebugMenuId
    {
        let mut name_hasher = std::hash::DefaultHasher::new();
        name.hash(&mut name_hasher);
        // TypeId::of::<T>().hash(&mut name_hasher);
        DebugMenuId(name_hasher.finish())
    }

    #[inline]
    pub fn gui_id<T: DebugGuiBase>(gui: &T) -> DebugMenuId
    {
        Self::gui_id_by_name::<T>(gui.name())
    }

    fn get_or_create_state<T: DebugGuiBase>(&mut self, gui: &T) -> &mut DebugMenuItemState
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
    pub fn set_state_active<T: DebugGuiBase>(&mut self, gui: &T, activate: bool)
    {
        let state = self.get_or_create_state(gui);
        if state.is_active != activate
        {
            state.is_active = activate;
            self.generation += 1;
        }
    }

    pub fn set_state_active_by_name<T: DebugGuiBase>(&mut self, name: &str, activate: bool)
    {
        let gui_id = Self::gui_id_by_name::<T>(name);
        let state = self.states.entry(gui_id.0).or_insert_with(||
        {
            DebugMenuItemState
            {
                name: name.to_string(),
                is_active: false
            }
        });
        if state.is_active != activate
        {
            state.is_active = activate;
            self.generation += 1;
        }
    }

    // toggle a state in the memory, does nothing if the state doesn't already exist. Returns the new state
    pub fn toggle_state_active<T: DebugGuiBase>(&mut self, gui: &T) -> Option<bool>
    {
        let gui_id = Self::gui_id(gui);
        match self.states.get_mut(&gui_id.0)
        {
            None => None,
            Some(state) =>
            {
                state.is_active ^= true;
                self.generation += 1;
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
    pub fn new(memory: &'m mut DebugMenuMemory, debug_gui: &egui::Context) -> Self
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
    pub fn add<T: DebugGuiBase>(&mut self, gui: &T)
    {
        // todo: sorted dict

        let state = self.memory.get_or_create_state(gui);
        gui.debug_gui_base(&mut state.is_active, &self.debug_gui)
    }
}