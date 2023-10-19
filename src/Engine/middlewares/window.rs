use std::{thread::{self, JoinHandle}, sync::Arc};
use parking_lot::Mutex;
use crate::{core_types::TickCount, Engine::{app::AppContext, core_types::CompletionState, middleware::Middleware}};

use glam::Vec2;
use winit::{
    event::*,
    event_loop::*,
    window::*,
    dpi::LogicalSize,
    error::OsError, platform::windows::EventLoopBuilderExtWindows,
};

const MAX_KEYCODE_ENTRIES: usize = 256;
const MAX_MOUSE_BUTTON_ENTRIES: usize = 5; /* TODO: don't hardcode */

struct WindowMiddlewareInternal
{
    is_exiting: bool,
}
impl Default for WindowMiddlewareInternal
{
    fn default() -> Self { Self
    {
        is_exiting: false,
    }}
}

pub struct WindowMiddleware
{
    input_thread: Option<JoinHandle<()>>,
    internal: Arc<Mutex<WindowMiddlewareInternal>>, // use thread scope instead of Arc?
}
impl WindowMiddleware
{
    pub fn new() -> Self { Self
    {
        input_thread: None,
        internal: Arc::new(Mutex::new(Default::default())),
    }}
}
impl Middleware for WindowMiddleware
{
    fn name(&self) -> &str { "Windows" }

    fn startup(&mut self, app: &mut AppContext) -> CompletionState
    {
        let internal_wrapper = self.internal.clone();
        let input_thread = thread::spawn(||
        {
            let event_loop = EventLoopBuilder::new()
                .with_any_thread(true)
                .build();

            // todo: don't do this this way?
            {
                let mut internal = internal_wrapper.lock();
                let main_window = WindowBuilder::new()
                    .with_inner_size(LogicalSize::new(1920, 1080))
                    .with_title("3L14")
                    .build(&event_loop).unwrap(); // TODO: don't unwrap

                //app.globals.try_add(main_window);
            }

            event_loop.run(move |event, _, control_flow|
            {
                let mut internal = internal_wrapper.lock();
                if internal.is_exiting
                {
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                match event
                {
                    winit::event::Event::WindowEvent { event: WindowEvent::CloseRequested, .. } =>
                    {
                        internal.is_exiting = true;
                    },
                    //winit::event::Event::NewEvents
                    //winit::event::Event::MainEventsCleared
                    _ => (),
                }
            });
        });
        self.input_thread = Some(input_thread);

        CompletionState::Completed
    }

    fn shutdown(&mut self, _app: &mut AppContext) -> CompletionState
    {
        match &self.input_thread
        {
            Some(it) if !it.is_finished() =>
            {
                // todo: timeout
                let mut internal = self.internal.lock();
                internal.is_exiting = true;
                CompletionState::InProgress
            },
            _ => CompletionState::Completed,
        }
    }

    fn run(&mut self, _app: &mut AppContext) -> CompletionState
    {
        let internal = self.internal.lock();
        match internal.is_exiting
        {
            true => CompletionState::Completed,
            _ => CompletionState::InProgress,
        }
    }
}


pub enum EventHandled
{
    Continue,
    Handled,
}

pub struct CreateWindow<'a>
{
    pub width: u32,
    pub height: u32,
    pub title: &'a str,
}

// global
pub struct Windows
{
    pub main_window: Window,
    pub input: WindowInputState,
}
impl Windows
{
    pub fn new<'e>(create_main_window: CreateWindow, event_loop: &'e EventLoop<()>) -> Result<Self, OsError>
    {
        let main_window = WindowBuilder::new()
            .with_title(create_main_window.title)
            .with_inner_size(LogicalSize { width: create_main_window.width, height: create_main_window.height })
            .build(event_loop)?;

        println!("Created main window with ID {:?}", main_window.id());

        Ok(Self
        {
            main_window: main_window,
            input: Default::default(),
        })
    }

    pub fn handle_input(&mut self, event: Event<'_, ()>, time: TickCount) -> EventHandled
    {
        match event
        {
            winit::event::Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } =>
            {
                self.input.mouse_state.move_delta.x = delta.0 as f32;
                self.input.mouse_state.move_delta.y = delta.1 as f32;
                self.input.mouse_state.position += self.input.mouse_state.move_delta;
                EventHandled::Handled
            }
            winit::event::Event::DeviceEvent { event: DeviceEvent::MouseWheel { delta }, .. } =>
            {
                match delta
                {
                    MouseScrollDelta::LineDelta(_x, _y) => todo!(),
                    MouseScrollDelta::PixelDelta(pixels) =>
                    {
                        self.input.mouse_state.wheel.x = pixels.x as f32;
                        self.input.mouse_state.wheel.y = pixels.y as f32;
                    }
                }
                EventHandled::Handled
            }
            winit::event::Event::DeviceEvent { event: DeviceEvent::Button { button, state }, .. } =>
            {
                let button = &mut self.input.mouse_state.buttons[button as usize];
                button.state.set(state == ElementState::Pressed);
                button.last_set_time = time;
                EventHandled::Handled
            }
            _ => EventHandled::Continue,
        }
    }
}

#[derive(Default, Copy, Clone, PartialEq)]
pub enum InputState
{
    #[default]
    Off,
    JustOn, // off->on this frame
    JustOff, // on->off this frame
    On,
    // repeat?
}
impl InputState
{
    pub fn set(&mut self, is_on: bool)
    {
        *self = match *self
        {
            InputState::Off if is_on => InputState::JustOn,
            InputState::JustOn if is_on => InputState::On,
            InputState::JustOn if !is_on => InputState::JustOff,
            InputState::JustOff if !is_on => InputState::Off,
            InputState::JustOff if is_on => InputState::JustOn,
            InputState::On if !is_on => InputState::JustOff,
            _ => todo!(),
        }
    }
}

#[derive(Copy, Clone)]
pub struct KeyState
{
    pub state: InputState,
    pub last_set_time: TickCount,
}
impl Default for KeyState
{
    fn default() -> Self {
        Self
        {
            state: InputState::Off,
            last_set_time: TickCount(0), // use time?
        }
    }
}

#[derive(Default, Copy, Clone)]
pub struct MouseButtonState
{
    pub state: InputState,
    pub last_set_time: TickCount,
}

#[derive(Copy, Clone, PartialEq)]
pub enum MouseButton
{
    Left    = 0,
    Middle  = 1,
    Right   = 2,
    X1      = 3,
    X2      = 4,
}

#[derive(Default, Copy, Clone)]
pub struct MouseState
{
    pub position: Vec2,
    pub move_delta: Vec2,

    pub buttons: [MouseButtonState; MAX_MOUSE_BUTTON_ENTRIES],
    pub wheel: Vec2,
}
impl MouseState
{
    pub fn button(&self, button: MouseButton) -> MouseButtonState
    {
        self.buttons[button as usize]
    }
    pub fn is_button_down(&self, button: MouseButton) -> bool
    {
        match self.buttons[button as usize].state
        {
            InputState::JustOn|InputState::On => true,
            _ => false,
        }
    }
    pub fn is_button_up(&self, button: MouseButton) -> bool
    {
        match self.buttons[button as usize].state
        {
            InputState::JustOff|InputState::Off => true,
            _ => false,
        }
    }
}

pub struct KeyboardState
{
    pub keys: [KeyState; MAX_KEYCODE_ENTRIES],
    pub modifiers: ModifiersState,
}
impl KeyboardState
{
    pub fn key(&self, key_code: VirtualKeyCode) -> KeyState { self.keys[key_code as usize] }
    pub fn is_key_down(&self, key_code: VirtualKeyCode) -> bool
    {
        match self.keys[key_code as usize].state
        {
            InputState::JustOn|InputState::On => true,
            _ => false,
        }
    }
    pub fn is_key_up(&self, key_code: VirtualKeyCode) -> bool
    {
        match self.keys[key_code as usize].state
        {
            InputState::JustOff|InputState::Off => true,
            _ => false,
        }
    }
    pub fn set_key(&mut self, key_code: VirtualKeyCode, is_pressed: bool)
    {
        self.keys[key_code as usize].state = match self.keys[key_code as usize].state
        {
            InputState::Off if is_pressed => InputState::JustOn,
            InputState::JustOn if is_pressed => InputState::On,
            InputState::JustOn if !is_pressed => InputState::JustOff,
            InputState::JustOff if !is_pressed => InputState::Off,
            InputState::JustOff if is_pressed => InputState::JustOn,
            InputState::On if !is_pressed => InputState::JustOff,
            no_change => no_change,
        }
    }
}
impl Default for KeyboardState
{
    fn default() -> Self { Self
    {
        keys: [KeyState::default(); MAX_KEYCODE_ENTRIES],
        modifiers: ModifiersState::empty(),
    }}
}

#[derive(Default)]
pub struct WindowInputState
{
    pub mouse_state: MouseState,
    pub keyboard_state: KeyboardState,
}