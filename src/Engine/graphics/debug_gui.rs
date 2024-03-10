pub trait DebugGui
{
    fn debug_gui(&self, context: &egui::Context);
}