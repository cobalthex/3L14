use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::Path;
use egui::Pos2;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use crate::debug_gui::DebugGuiBase;

pub struct DebugMenuId(u64);

#[derive(Serialize, Deserialize)]
struct DebugMenuItemState
{
    name: String,
    is_active: bool,
}

mod serialize_debug_menu_states
{
    use std::fmt::Formatter;
    use std::hash::DefaultHasher;
    use super::*;
    use serde::{Deserializer, Serializer};
    use serde::de::{MapAccess, Visitor};
    use serde::ser::SerializeMap;

    pub fn serialize<S: Serializer>(mem: &IndexMap<u64, DebugMenuItemState>, serializer: S) -> Result<S::Ok, S::Error>
    {
        let mut s = serializer.serialize_map(Some(mem.len()))?;
        for (_, state) in mem
        {
            s.serialize_entry(&state.name, &state.is_active)?;
        }
        s.end()
    }


    struct StatesVisitor;
    impl<'de> Visitor<'de> for StatesVisitor
    {
        type Value = IndexMap<u64, DebugMenuItemState>;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result
        {
            formatter.write_str("a map with an 'id' field and key-value pairs")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error>
        {
            let mut imap = IndexMap::new();

            while let Some(key) = map.next_key::<String>()?
            {
                let key_hash =
                {
                    let mut hasher = DefaultHasher::new();
                    key.hash(&mut hasher);
                    hasher.finish()
                };
                let value = map.next_value()?;
                imap.insert(key_hash, DebugMenuItemState
                {
                    name: key,
                    is_active: value,
                });
            }

            Ok(imap)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IndexMap<u64, DebugMenuItemState>, D::Error>
    {
        deserializer.deserialize_map(StatesVisitor)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct DebugMenuMemory
{
    #[serde(skip)]
    generation: usize, // updated everytime a state changes
    #[serde(skip)]
    last_saved_generation: usize,

    is_active: bool, // is the top level menu active
    #[serde(with = "serialize_debug_menu_states")]
    states: IndexMap<u64, DebugMenuItemState>, // todo: this should be sorted
}
impl DebugMenuMemory
{
    #[inline] #[must_use] pub fn generation(&self) -> usize { self.generation }

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

    #[must_use]
    pub fn load(path: impl AsRef<Path>) -> Self
    {
        let mut fread = match std::fs::File::open(&path)
        {
            Ok(file) => file,
            Err(e) =>
                {
                    log::warn!("Failed to open debug GUI state file '{:?}' for reading: {e}", path.as_ref());
                    return Self::default();
                },
        };
        let mut toml = String::new();
        if let Err(e) = fread.read_to_string(&mut toml)
        {
            log::warn!("Failed to read debug GUI state from file: {e}");
            return Self::default();
        };

        toml::from_str(&toml).unwrap_or_else(|_| Self::default())
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
                        let is_active = gui.is_active;
                        ui.checkbox(&mut gui.is_active, &gui.name);
                        if gui.is_active != is_active
                        {
                            self.memory.generation += 1;
                        }
                    }
                });
    }

    // todo: accept slice? (might need some hlist magic)
    pub fn add<T: DebugGuiBase>(&mut self, gui: &T)
    {
        // todo: sorted dict

        let state = self.memory.get_or_create_state(gui);
        let is_active = state.is_active;
        gui.debug_gui_base(&mut state.is_active, &self.debug_gui);
        if state.is_active != is_active
        {
            self.memory.generation += 1;
        }
    }
}