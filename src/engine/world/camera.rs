use std::time::Duration;
use super::{Transform, ViewMtx};
use crate::engine::graphics::debug_gui::DebugGui;
use crate::engine::math::{Plane, Radians};
use egui::Ui;
use glam::{Mat4, Vec4};
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

    pub transform: Transform,

    pub projection: CameraProjection,
    pub near_clip: f32,
    pub far_clip: f32,

    view_mtx: ViewMtx,
    projection_mtx: ProjectionMtx,
    frustum: Frustum,
}
impl Camera
{
    pub fn new(name: Option<impl AsRef<str>>, projection: CameraProjection) -> Self
    {
        let near_clip =  0.1;
        let far_clip = 1000.0;

        let transform = Transform::default();
        let view_mtx = transform.to_view();
        let projection_mtx = projection.as_matrix(near_clip, far_clip);
        let frustum = todo!();

        Self
        {
            name: name.map(|n| n.as_ref().to_string()),
            transform,
            projection,
            near_clip,
            far_clip,
            view_mtx,
            projection_mtx,
            frustum,
        }
    }

    pub fn view(&self) -> ViewMtx { self.view_mtx }
    pub fn projection(&self) -> ProjectionMtx { self.projection_mtx }

    pub fn update_view(&mut self) -> &ViewMtx
    {
        self.view_mtx = self.transform.to_view();
        &self.view_mtx
    }
    pub fn update_projection(&mut self) -> &ProjectionMtx
    {
        self.projection_mtx = self.projection.as_matrix(self.near_clip, self.far_clip);
        &self.projection_mtx
    }
}
impl DebugGui for Camera
{
    fn name(&self) -> &str { "Camera" } // TODO { format!("Camera '{}'", self.name.as_ref().map_or("", |n| n.as_str())) }

    fn debug_gui(&self, ui: &mut Ui)
    {
        ui.label(format!("Position: {:.2?}", self.transform.position));
        ui.label(format!("Forward: {:.2?}", self.transform.forward()));
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
