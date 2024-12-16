use egui::Ui;
use glam::Mat4;
use gltf::camera::Projection;
use crate::engine::{Radians, Degrees};
use crate::engine::graphics::debug_gui::DebugGui;
use super::{Transform, ViewMtx};

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
}
impl Camera
{
    pub fn new<S: AsRef<str>>(name: Option<S>, projection: CameraProjection) -> Self
    {
        let near_clip =  0.1;
        let far_clip = 1000.0;

        let transform = Transform::default();
        let view_mtx = transform.to_view();
        let projection_mtx = projection.as_matrix(near_clip, far_clip);

        Self
        {
            name: name.map(|n| n.as_ref().to_string()),
            transform,
            projection,
            near_clip,
            far_clip,
            view_mtx,
            projection_mtx,
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
    pub total_secs: f32,
}

impl CameraUniform
{
    pub fn new(camera: &Camera, clock: &crate::engine::timing::Clock) -> Self
    {
        Self
        {
            proj_view: camera.projection().0 * camera.view().0,
            total_secs: clock.total_runtime().as_secs_f32(),
        }
    }
}
