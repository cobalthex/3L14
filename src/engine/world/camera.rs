use std::time::Duration;
use super::{Transform, WORLD_FORWARD, WORLD_UP};
use crate::engine::graphics::debug_gui::DebugGui;
use crate::engine::math::{Plane, Radians};
use egui::Ui;
use glam::{Mat4, Vec3, Vec4};
use super::Frustum;

#[derive(Debug, Clone)]
pub enum CameraProjection
{
    Perspective
    {
        fov: Radians,
        aspect_ratio: f32,
    },
    Orthographic
    {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    },
}
impl CameraProjection
{
    pub fn as_matrix(&self, near_clip: f32, far_clip: f32) -> Mat4
    {
        match self
        {
            CameraProjection::Perspective { fov, aspect_ratio } =>
            {
                Mat4::perspective_lh(fov.0, *aspect_ratio, near_clip, far_clip)
            },
            CameraProjection::Orthographic { left, top, right, bottom } =>
            {
                Mat4::orthographic_lh(*left, *right, *bottom, *top, near_clip, far_clip)
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Camera
{
    projection: CameraProjection, // todo: store near/far clip
    near_clip: f32,
    far_clip: f32,

    // should view/proj matrices be stored?

    transform: Transform,
    clip_mtx: Mat4,
}
impl Camera
{
    pub fn transform(&self) -> &Transform { &self.transform }
    pub fn clip_mtx(&self) -> Mat4 { self.clip_mtx }

    // Update cached values after updating one of the public fields
    pub fn update_projection(&mut self, projection: CameraProjection, near_clip: f32, far_clip: f32)
    {
        self.projection = projection;
        self.near_clip = near_clip;
        self.far_clip = far_clip;
        let projection_mtx = self.projection.as_matrix(self.near_clip, self.far_clip);
        self.clip_mtx = projection_mtx * self.transform.to_view();
    }

    pub fn update_view(&mut self, transform: Transform)
    {
        self.transform = transform;
        self.clip_mtx = self.projection.as_matrix(self.near_clip, self.far_clip) * self.transform.to_view();
    }
}
impl Default for Camera
{
    fn default() -> Self
    {
        let projection = CameraProjection::Perspective { fov: Radians(90.0), aspect_ratio: 16.0 / 9.0 };
        let transform = Transform::default();
        let clip_mtx = projection.as_matrix(0.1, 1000.0) * transform.to_view();

        Self
        {
            projection,
            near_clip: 0.1,
            far_clip: 1000.0,
            transform,
            clip_mtx,
        }
    }
}
impl DebugGui for Camera
{
    fn name(&self) -> &str { "Camera" }

    fn debug_gui(&self, ui: &mut Ui)
    {
        #[cfg(debug_assertions)]
        {
            // TODO
            // ui.label(format!("Position: {:.2?}", self.debug_transform.position));
            // ui.label(format!("Forward: {:.2?}", self.debug_transform.forward()));
        }
        ui.label(format!("Projection: {:?}", self.projection));
    }
}

#[repr(C, align(16))]
pub struct CameraUniform
{
    pub proj_view: Mat4,
    pub total_secs_whole: u32,
    pub total_secs_frac: f32,
}

impl CameraUniform
{
    pub fn new(proj_view: Mat4, runtime: Duration) -> Self
    {
        let runtime_millis = runtime.as_millis();

        Self
        {
            proj_view,
            total_secs_whole: (runtime_millis / 1000) as u32,
            total_secs_frac: (runtime_millis % 1000) as f32 / 1000.0,
        }
    }
}
