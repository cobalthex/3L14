use std::time::Duration;
use egui::Ui;
use glam::{Mat4, Vec3};
use debug_3l14::debug_gui::DebugGui;
use math_3l14::{Frustum, Radians, Transform};

#[derive(Debug, Clone)]
pub enum CameraProjection
{
    Perspective
    {
        fov: Radians,
        aspect_ratio: f32, // width / height
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
    pub fn to_matrix(&self, near_clip: f32, far_clip: f32) -> Mat4
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

    pub fn aspect_ratio(&self) -> f32
    {
        match self
        {
            CameraProjection::Perspective { aspect_ratio, .. } => *aspect_ratio,
            CameraProjection::Orthographic { top, left, right, bottom } =>
            {
                (right - left) / (bottom - top)
            }
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
    #[inline] #[must_use] pub fn transform(&self) -> &Transform { &self.transform }
    #[inline] #[must_use] pub fn projection(&self) -> &CameraProjection { &self.projection }
    #[inline] #[must_use] pub fn matrix(&self) -> Mat4 { self.clip_mtx } // todo: better name?

    #[inline] #[must_use] pub fn near_clip(&self) -> f32 { self.near_clip }
    #[inline] #[must_use] pub fn far_clip(&self) -> f32 { self.far_clip }

    // Update cached values after updating one of the public fields
    pub fn update_projection(&mut self, projection: CameraProjection, near_clip: f32, far_clip: f32)
    {
        self.projection = projection;
        self.near_clip = near_clip;
        self.far_clip = far_clip;
        let projection_mtx = self.projection.to_matrix(self.near_clip, self.far_clip);
        self.clip_mtx = projection_mtx * self.transform.to_view_mtx();
    }

    pub fn update_view(&mut self, transform: Transform)
    {
        self.transform = transform;
        self.clip_mtx = self.projection.to_matrix(self.near_clip, self.far_clip) * self.transform.to_view_mtx();
    }
}
impl Default for Camera
{
    fn default() -> Self
    {
        let projection = CameraProjection::Perspective { fov: Radians(90.0), aspect_ratio: 16.0 / 9.0 };
        let transform = Transform::default();
        let clip_mtx = projection.to_matrix(0.1, 1000.0) * transform.to_view_mtx();

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
        match self.projection
        {
            CameraProjection::Perspective { fov, aspect_ratio } =>
            {
                ui.heading("Perspective");
                ui.label(format!("FOV: {:.02}\naspect ratio: {:.02}", fov.to_degrees(), aspect_ratio));
            },
            CameraProjection::Orthographic { left, top, right, bottom } =>
            {
                ui.heading("Orthographic");
                ui.label(format!("left: {}\ntop: {}\nright: {}\nbottom: {}", left, top, right, bottom));
            }
        };
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
