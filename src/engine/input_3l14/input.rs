use std::time::Instant;
use glam::IVec2;
use sdl2::event::Event;
use sdl2::keyboard::Mod;
use debug_3l14::debug_gui::DebugGui;
use super::*;

#[derive(Debug)]
pub enum WhichInput
{
    None, // only valid on startup
    Mouse,
    Keyboard,
    Controller,
}

#[derive(Debug)]
pub struct LastInput
{
    pub which: WhichInput,
    pub time: Instant,
    // device ID
}

#[derive(Debug)]
pub struct Input
{
    last_input: LastInput,

    // per-user devices?

    controller: ControllerState,
    keyboard: KeyboardState,
    mouse: MouseState,
}

impl Input
{
    pub fn new(sdl: &sdl2::Sdl) -> Self
    {
        Self
        {
            last_input: LastInput
            {
                which: WhichInput::None,
                time: Instant::now(),
            },
            mouse: MouseState::new(sdl.mouse()),
            keyboard: KeyboardState::default(),
            controller: ControllerState::default(),
        }
    }

    pub fn controller(&self) -> &ControllerState { &self.controller }
    pub fn keyboard(&self) -> &KeyboardState { &self.keyboard }
    pub fn mouse(&self) -> &MouseState { &self.mouse }

    pub fn pre_update(&mut self)
    {
        puffin::profile_function!();
        self.controller.pre_update();
        self.keyboard.pre_update();
        self.mouse.pre_update();
    }

    pub fn handle_event(&mut self, event: Event, time: Instant)
    {
        match event
        {
            Event::KeyDown { keycode, scancode, keymod, .. } =>
            {
                if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD)
                {
                    self.keyboard.mods |= KeyMods::CTRL;
                }
                if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD)
                {
                    self.keyboard.mods |= KeyMods::SHIFT;
                }
                if keymod.intersects(Mod::LALTMOD | Mod::RALTMOD)
                {
                    self.keyboard.mods |= KeyMods::ALT;
                }

                if let Some(key) = keycode
                {
                    if self.keyboard.get_key(key).is_none()
                    {
                        self.keyboard.pressed_keys.push(KeyState
                        {
                            key_code: key,
                            scan_code: scancode.unwrap_or(unsafe { std::mem::transmute(0) }),
                            state: ButtonState::JustOn,
                            set_time: time,
                        });
                    }
                }
            }
            Event::KeyUp { keycode, keymod, .. } =>
            {
                if keymod.intersection(Mod::LCTRLMOD | Mod::RCTRLMOD).is_empty()
                {
                    self.keyboard.mods -= KeyMods::CTRL;
                }
                if keymod.intersection(Mod::LSHIFTMOD | Mod::RSHIFTMOD).is_empty()
                {
                    self.keyboard.mods -= KeyMods::SHIFT;
                }
                if keymod.intersection(Mod::LALTMOD | Mod::RALTMOD).is_empty()
                {
                    self.keyboard.mods -= KeyMods::ALT;
                }

                match keycode
                {
                    Some(key) =>
                        {
                            if let Some(keystate) = self.keyboard.get_key_mut(key)
                            {
                                keystate.state = ButtonState::JustOff
                            }
                        }
                    None => panic!("invalid on->off state transition"), // todo
                }
            }

            // Event::TextEditing { .. } => {}
            // Event::TextInput { .. } => {}

            Event::MouseMotion { x, y, xrel, yrel, .. } =>
            {
                self.mouse.position = IVec2::new(x, y);
                self.mouse.position_delta = IVec2::new(xrel, yrel);
            }
            Event::MouseButtonDown { mouse_btn, .. } => // double click?
            {
                let button = &mut self.mouse.buttons[(mouse_btn as usize) - 1];
                button.state = ButtonState::JustOn;
                button.set_time = Some(time);
            }
            Event::MouseButtonUp { mouse_btn, .. } =>
            {
                let button = &mut self.mouse.buttons[(mouse_btn as usize) - 1];
                button.state = ButtonState::JustOff;
            }
            Event::MouseWheel { x, y, .. } =>
            {
                // precise x/y?
                self.mouse.wheel += IVec2::new(x, y);
                self.mouse.wheel_delta = IVec2::new(x, y);
            }

            // Event::JoyAxisMotion { .. } => {}
            // Event::JoyBallMotion { .. } => {}
            // Event::JoyHatMotion { .. } => {}
            // Event::JoyButtonDown { .. } => {}
            // Event::JoyButtonUp { .. } => {}
            // Event::JoyDeviceAdded { .. } => {}
            // Event::JoyDeviceRemoved { .. } => {}
            //
            Event::ControllerAxisMotion { which, axis, value, .. } =>
            {

            }
            // Event::ControllerButtonDown { .. } => {}
            // Event::ControllerButtonUp { .. } => {}
            // Event::ControllerDeviceAdded { .. } => {}
            // Event::ControllerDeviceRemoved { .. } => {}
            // Event::ControllerDeviceRemapped { .. } => {}
            // Event::ControllerSensorUpdated { .. } => {}
            //
            // Event::FingerDown { .. } => {}
            // Event::FingerUp { .. } => {}
            // Event::FingerMotion { .. } => {}
            //
            // Event::DollarGesture { .. } => {}
            // Event::DollarRecord { .. } => {}
            // Event::MultiGesture { .. } => {}

            _ => {}
        }
    }

    pub fn into_egui(&self, zoom_scale_factor: f32) -> egui::RawInput
    {
        let mut ri = egui::RawInput::default();
        ri.modifiers.ctrl = self.keyboard.has_keymod(KeyMods::CTRL);
        ri.modifiers.shift = self.keyboard.has_keymod(KeyMods::SHIFT);
        ri.modifiers.alt = self.keyboard.has_keymod(KeyMods::ALT);

        let mouse_pos = egui::Pos2
        {
            x: self.mouse.position.x as f32 / zoom_scale_factor,
            y: self.mouse.position.y as f32 / zoom_scale_factor,
        };

        ri.events.push(egui::Event::PointerMoved(mouse_pos));

        ri.events.push(egui::Event::MouseWheel
        {
            delta: egui::Vec2
            {
                x: self.mouse.wheel_delta.x as f32,
                y: self.mouse.wheel_delta.y as f32
            },
            unit: egui::MouseWheelUnit::Point,
            modifiers: ri.modifiers,
        });

        for i in 0..self.mouse.buttons.len()
        {
            let pressed = match self.mouse.buttons[i].state
            {
                ButtonState::JustOn|ButtonState::On => true,
                ButtonState::JustOff => false,
                ButtonState::Off => continue,
            };

            ri.events.push(egui::Event::PointerButton
            {
                pos: mouse_pos,
                button: match i
                {
                    0 => egui::PointerButton::Primary,
                    1 => egui::PointerButton::Middle,
                    2 => egui::PointerButton::Secondary,
                    3 => egui::PointerButton::Extra1,
                    4 => egui::PointerButton::Extra2,
                    _ => panic!("Unknown pointer button")
                },
                pressed,
                modifiers: ri.modifiers,
            })
        }

        // todo: keyboard events
        // todo: other events

        ri
    }
}

impl DebugGui for Input
{
    fn display_name(&self) -> &str { "Input state" }
    fn debug_gui(&self, ui: &mut egui::Ui)
    {
        ui.horizontal_top(|hui|
            {
                hui.collapsing("Keyboard", |kbui|
                    {
                        kbui.set_min_width(120.0);
                        kbui.label(format!("Mods: {:?}", self.keyboard.mods));
                        let mut any = false;
                        for state in self.keyboard.pressed_keys.iter()
                        {
                            any = true;
                            kbui.label(format!("{:?}: {:?}", state.key_code, state.state));
                        }
                        if !any
                        {
                            kbui.label("(No keys pressed)");
                        }
                    });

                hui.collapsing("Mouse", |mui|
                    {
                        mui.set_min_width(200.0);
                        mui.label(format!("Pos: {:?} - Delta: {:?}", self.mouse.position.to_array(), self.mouse.position_delta.to_array()));
                        mui.label(format!("Wheel: {:?} - Delta: {:?}", self.mouse.wheel.to_array(), self.mouse.wheel_delta.to_array()));
                        mui.label(format!("LB: {:?}", self.mouse.get_button(MouseButton::Left).state));
                        mui.label(format!("MB: {:?}", self.mouse.get_button(MouseButton::Middle).state));
                        mui.label(format!("RB: {:?}", self.mouse.get_button(MouseButton::Right).state));
                        mui.label(format!("X1: {:?}", self.mouse.get_button(MouseButton::X1).state));
                        mui.label(format!("X2: {:?}", self.mouse.get_button(MouseButton::X2).state));
                    });
            });
    }
}
