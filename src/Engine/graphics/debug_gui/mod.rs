use egui::{Context, Ui, Window};
use crate::engine::FrameNumber;

pub trait DebugGuiBase<'n>
{
    // This should be a unique name
    fn name(&self) -> &'n str;

    fn debug_gui_base(&self, is_active: &mut bool, debug_gui: &egui::Context);
}

pub trait DebugGui<'n>
{
    // This should be a unique name
    fn name(&self) -> &'n str;

    fn debug_gui(&self, ui: &mut Ui);
}
impl<'n, T: DebugGui<'n>> DebugGuiBase<'n> for T
{
    fn name(&self) -> &'n str {
        self.name()
    }

    fn debug_gui_base(&self, is_active: &mut bool, debug_gui: &Context)
    {
        Window::new(self.name())
            .movable(true)
            .resizable(true)
            .open(is_active)
            .show(debug_gui, |ui| self.debug_gui(ui));
    }
}

pub mod debug_menu;
pub mod sparkline;

pub struct FrameProfiler;
impl<'n> DebugGuiBase<'n> for FrameProfiler
{
    fn name(&self) -> &'n str { "Frame Profiler" }

    fn debug_gui_base(&self, is_active: &mut bool, debug_gui: &Context)
    {
        if *is_active
        {
            *is_active = puffin_egui::profiler_window(&debug_gui);
            // note: can also call profiler_ui
        }
    }
}

pub struct AppStats
{
    pub fps: f32,
    pub frame_number: FrameNumber,
    pub app_runtime: f64,

    pub main_window_size: (u32, u32),
    pub viewport_size: (u32, u32),
}
impl<'n> DebugGuiBase<'n> for AppStats
{
    fn name(&self) -> &'n str { "App Stats" }

    fn debug_gui_base(&self, is_active: &mut bool, debug_gui: &Context)
    {
        Window::new(self.name())
            .movable(true)
            .resizable(true)
            .title_bar(false)
            .open(is_active)
            .show(debug_gui, |ui|
                {
                    ui.label(format!("FPS: {:.1}", self.fps));
                    // ui.add(&fps_sparkline); // todo

                    ui.label(format!("Frame: {}", self.frame_number));
                    ui.label(format!("App time: {:.1}s", self.app_runtime));
                    ui.label(format!("Window: {} x {}", self.main_window_size.0, self.main_window_size.1));
                    ui.label(format!("Viewport: {} x {}", self.viewport_size.0, self.viewport_size.1));
                });
    }
}