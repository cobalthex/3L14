// use std::{thread::{self, JoinHandle}, sync::Arc};
// use crate::engine::{middleware::*, core_types::CompletionState};
// use sdl2::{*, video::*, event::Event, keyboard::Keycode};

// use glam::Vec2;

// const MAX_KEYCODE_ENTRIES: usize = 256;
// const MAX_MOUSE_BUTTON_ENTRIES: usize = 5; /* TODO: don't hardcode */

// struct WindowMiddlewareInternal
// {
//     is_exiting: bool,
// }
// impl Default for WindowMiddlewareInternal
// {
//     fn default() -> Self { Self
//     {
//         is_exiting: false,
//     }}
// }

// pub struct CreateWindow<'a>
// {
//     pub width: u32,
//     pub height: u32,
//     pub title: &'a str,
// }

// pub struct WindowMiddleware
// {
//     sdl_context: Sdl,
//     sdl_video: VideoSubsystem,
//     sdl_timer: TimerSubsystem,
// }

// pub struct WindowMiddlewareGlobals
// {
//     sdl_events: EventPump,
// }

// static wm_globals: Option<WindowMiddlewareGlobals> = None;
// impl Globals<WindowMiddlewareGlobals> for WindowMiddleware
// {
//     fn init<'a>() -> &'a mut Self
//     {
//         if wm_globals.is_some()
//         {
//             panic!("Cannot initialize twice");
//         }
//         wm_globals = Some(Self::new());
//         wm_globals
//     }
//     fn uninit()
//     {
//         wm_globals = None;
//     }
//     fn get<'a>() -> Option<&'a mut Self> { wm_globals }
// }

// impl WindowMiddleware
// {
//     fn new() -> Self
//     {
//         let sdl = sdl2::init().unwrap();
//         let video = sdl.video().unwrap();
//         let timer = sdl.timer().unwrap();
//         let events = sdl.event_pump().unwrap();
//         Self
//         {
//             sdl_context: sdl,
//             sdl_video: video,
//             sdl_timer: timer,
//             sdl_events: events,
//         }
//     }

//     pub fn create_window(&self, create_window: CreateWindow) -> Result<Window, ()> // TODO: error type
//     {
//         self.sdl_video.window(create_window.title, create_window.width, create_window.height)
//             .position_centered()
//             .build()
//             .map_err(|e| ()) // TODO
//     }
// }
// impl Middleware for WindowMiddleware
// {
//     fn startup(&mut self) -> CompletionState
//     {
//         CompletionState::Completed
//     }

//     fn shutdown(&mut self) -> CompletionState
//     {
//         CompletionState::Completed
//     }

//     fn run(&mut self) -> CompletionState
//     {
//         for event in self.sdl_events.poll_iter()
//         {
//             match event
//             {
//                 Event::Quit {..} =>
//                 {
//                     return CompletionState::Completed;
//                 },
//                 Event::KeyDown { timestamp, window_id, keycode, scancode, keymod, repeat } =>
//                 {

//                 }
//                 _ => {}
//             }
//         }
//         return CompletionState::InProgress;
//     }
// }

// // // // global
// // // pub struct Windows
// // // {
// // //     pub main_window: sdl2::video::Window,
// // //     pub input: WindowInputState,
// // // }
// // // impl Windows
// // // {
// // //     pub fn new<'e>(create_main_window: CreateWindow) -> Result<Self, () /* TODO */>
// // //     {
// // //         let main_window =

// // //         println!("Created main window with ID {:?}", main_window.id());

// // //         Ok(Self
// // //         {
// // //             main_window: main_window,
// // //             input: Default::default(),
// // //         })
// // //     }

// //     // pub fn handle_input(&mut self, event: Event<'_, ()>, time: TickCount) -> EventHandled
// //     // {
// //     //     match event
// //     //     {
// //     //         winit::event::Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } =>
// //     //         {
// //     //             self.input.mouse_state.move_delta.x = delta.0 as f32;
// //     //             self.input.mouse_state.move_delta.y = delta.1 as f32;
// //     //             self.input.mouse_state.position += self.input.mouse_state.move_delta;
// //     //             EventHandled::Handled
// //     //         }
// //     //         winit::event::Event::DeviceEvent { event: DeviceEvent::MouseWheel { delta }, .. } =>
// //     //         {
// //     //             match delta
// //     //             {
// //     //                 MouseScrollDelta::LineDelta(_x, _y) => todo!(),
// //     //                 MouseScrollDelta::PixelDelta(pixels) =>
// //     //                 {
// //     //                     self.input.mouse_state.wheel.x = pixels.x as f32;
// //     //                     self.input.mouse_state.wheel.y = pixels.y as f32;
// //     //                 }
// //     //             }
// //     //             EventHandled::Handled
// //     //         }
// //     //         winit::event::Event::DeviceEvent { event: DeviceEvent::Button { button, state }, .. } =>
// //     //         {
// //     //             let button = &mut self.input.mouse_state.buttons[button as usize];
// //     //             button.state.set(state == ElementState::Pressed);
// //     //             button.last_set_time = time;
// //     //             EventHandled::Handled
// //     //         }
// //     //         _ => EventHandled::Continue,
// //     //     }
// //     // }
// // //}

// // #[derive(Default, Copy, Clone, PartialEq)]
// // pub enum InputState
// // {
// //     #[default]
// //     Off,
// //     JustOn, // off->on this frame
// //     JustOff, // on->off this frame
// //     On,
// //     // repeat?
// // }
// // impl InputState
// // {
// //     pub fn set(&mut self, is_on: bool)
// //     {
// //         *self = match *self
// //         {
// //             InputState::Off if is_on => InputState::JustOn,
// //             InputState::JustOn if is_on => InputState::On,
// //             InputState::JustOn if !is_on => InputState::JustOff,
// //             InputState::JustOff if !is_on => InputState::Off,
// //             InputState::JustOff if is_on => InputState::JustOn,
// //             InputState::On if !is_on => InputState::JustOff,
// //             _ => todo!(),
// //         }
// //     }
// // }

// // #[derive(Copy, Clone)]
// // pub struct KeyState
// // {
// //     pub state: InputState,
// //     pub last_set_time: TickCount,
// // }
// // impl Default for KeyState
// // {
// //     fn default() -> Self {
// //         Self
// //         {
// //             state: InputState::Off,
// //             last_set_time: TickCount(0), // use time?
// //         }
// //     }
// // }

// // #[derive(Default, Copy, Clone)]
// // pub struct MouseButtonState
// // {
// //     pub state: InputState,
// //     pub last_set_time: TickCount,
// // }

// // #[derive(Copy, Clone, PartialEq)]
// // pub enum MouseButton
// // {
// //     Left    = 0,
// //     Middle  = 1,
// //     Right   = 2,
// //     X1      = 3,
// //     X2      = 4,
// // }

// // #[derive(Default, Copy, Clone)]
// // pub struct MouseState
// // {
// //     pub position: Vec2,
// //     pub move_delta: Vec2,

// //     pub buttons: [MouseButtonState; MAX_MOUSE_BUTTON_ENTRIES],
// //     pub wheel: Vec2,
// // }
// // impl MouseState
// // {
// //     pub fn button(&self, button: MouseButton) -> MouseButtonState
// //     {
// //         self.buttons[button as usize]
// //     }
// //     pub fn is_button_down(&self, button: MouseButton) -> bool
// //     {
// //         match self.buttons[button as usize].state
// //         {
// //             InputState::JustOn|InputState::On => true,
// //             _ => false,
// //         }
// //     }
// //     pub fn is_button_up(&self, button: MouseButton) -> bool
// //     {
// //         match self.buttons[button as usize].state
// //         {
// //             InputState::JustOff|InputState::Off => true,
// //             _ => false,
// //         }
// //     }
// // }
