use std::time::Duration;
use super::{Transform, ViewMtx, WORLD_FORWARD, WORLD_UP};
use crate::engine::graphics::debug_gui::DebugGui;
use crate::engine::math::{Plane, Radians};
use egui::Ui;
use glam::{Mat4, Vec3, Vec4};
use super::Frustum;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectionMtx(pub Mat4);

#[derive(Debug)]
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
    pub fn as_matrix(&self, near_clip: f32, far_clip: f32) -> ProjectionMtx
    {
        match self
        {
            CameraProjection::Perspective { fov, aspect_ratio } =>
            {
                ProjectionMtx(Mat4::perspective_lh(fov.0, *aspect_ratio, near_clip, far_clip))
            },
            CameraProjection::Orthographic { left, top, right, bottom } =>
            {
                ProjectionMtx(Mat4::orthographic_lh(*left, *right, *bottom, *top, near_clip, far_clip))
            },
        }
    }
}

#[derive(Debug)]
pub struct Camera
{
    pub name: Option<String>,

    #[cfg(debug_assertions)]
    debug_transform: Transform,

    projection: CameraProjection,

    view_mtx: ViewMtx,
    projection_mtx: ProjectionMtx,
    frustum: Frustum,
}
impl Camera
{
    pub fn new(name: Option<impl AsRef<str>>) -> Self
    {
        let projection = CameraProjection::Perspective { fov: Radians(90.0), aspect_ratio: 16.0 / 9.0 };
        let view_mtx = ViewMtx(Mat4::look_at_lh(Vec3::ZERO, WORLD_FORWARD, WORLD_UP));
        let projection_mtx = projection.as_matrix(0.1, 10.0);
        let frustum = Frustum::new(&(projection_mtx.0 * view_mtx.0)); // v*p?

        Self
        {
            name: name.map(|n| n.as_ref().to_string()),
            debug_transform: Transform::default(),
            projection,
            view_mtx,
            projection_mtx,
            frustum,
        }
    }

    pub fn view(&self) -> ViewMtx { self.view_mtx }
    pub fn projection(&self) -> ProjectionMtx { self.projection_mtx }
    pub fn frustum(&self) -> &Frustum { &self.frustum }

    // Update cached values after updating one of the public fields
    pub fn update_projection(&mut self, projection: CameraProjection, near_clip: f32, far_clip: f32)
    {
        self.projection = projection;
        self.projection_mtx = self.projection.as_matrix(near_clip, far_clip);
        self.frustum = Frustum::new(&(self.projection_mtx.0 * self.view_mtx.0)); // v*p?
    }

    pub fn update_view(&mut self, transform: &Transform)
    {
        self.view_mtx = transform.to_view();
        self.frustum = Frustum::new(&(self.projection_mtx.0 * self.view_mtx.0));
    }
}
impl DebugGui for Camera
{
    fn name(&self) -> &str { "Camera" } // TODO { format!("Camera '{}'", self.name.as_ref().map_or("", |n| n.as_str())) }

    fn debug_gui(&self, ui: &mut Ui)
    {
        #[cfg(debug_assertions)]
        {
            ui.label(format!("Position: {:.2?}", self.debug_transform.position));
            ui.label(format!("Forward: {:.2?}", self.debug_transform.forward()));
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
