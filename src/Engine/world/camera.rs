use egui::{Context, Ui};
use glam::Mat4;
use crate::engine::{Radians, Degrees};
use crate::engine::graphics::debug_gui::DebugGui;
use super::Transform;

pub struct Camera
{
    pub name: Option<String>,

    pub transform: Transform,

    pub fov: Radians,
    pub aspect_ratio: f32, // w / h
    pub near_clip: f32,
    pub far_clip: f32,

    view: Mat4,
    projection: Mat4,
}
impl Camera
{
    pub fn new<S: AsRef<str>>(name: Option<S>, aspect_ratio: f32) -> Self
    {
        let fov: Radians = Degrees(59.0).into(); // 90 deg horizontal FOV
        let near_clip =  0.1;
        let far_clip = 1000.0;

        let transform = Transform::default();
        let view = transform.to_view();
        Self
        {
            name: name.map(|n| n.as_ref().to_string()),
            transform,
            fov,
            aspect_ratio,
            near_clip,
            far_clip,
            view,
            projection: Mat4::perspective_lh(fov.0, aspect_ratio, near_clip, far_clip),
        }
    }

    pub fn view(&self) -> Mat4 { self.view }
    pub fn projection(&self) -> Mat4 { self.projection }

    pub fn update_view(&mut self) -> Mat4
    {
        self.view = self.transform.to_view();
        self.view
    }
    pub fn update_projection(&mut self) -> Mat4
    {
        self.projection = Mat4::perspective_lh(self.fov.0, self.aspect_ratio, self.near_clip, self.far_clip);
        self.projection
    }
}
impl<'n> DebugGui<'n> for Camera
{
    fn name(&self) -> &'n str { "Camera" } // TODO { format!("Camera '{}'", self.name.as_ref().map_or("", |n| n.as_str())) }

    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.label(format!("Position: {:.2?}", self.transform.position));
        ui.label(format!("Forward: {:.2?}", self.transform.forward()));
        // ui.label(format!("Right: {:.2?}", self.transform.right()));
        // ui.label(format!("Up: {:.2?}", self.transform.up()));
        ui.label(format!("FOV: {}", self.fov.to_degrees()));
    }
}

#[repr(packed(16))]
pub struct CameraUniform
{
    pub proj_view: Mat4,
    pub total_secs: f32,
}

impl CameraUniform
{
    pub fn new(camera: &Camera, clock: &crate::engine::timing::Clock) -> Self
    {
        Self
        {
            proj_view: camera.projection() * camera.view(),
            total_secs: clock.total_runtime().as_secs_f32(),
        }
    }
}
