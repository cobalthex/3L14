use glam::{Mat4, Vec3};
use crate::engine::{Radians, Degrees};
use super::Transform;

pub struct Camera
{
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
    pub fn new(aspect_ratio: f32) -> Self
    {
        let fov: Radians = Degrees(59.0).into(); // 90 deg horizontal FOV
        let near_clip =  0.1;
        let far_clip = 1000.0;

        let transform = Transform::default();
        let view = Mat4::look_to_lh(transform.position, transform.forward(), transform.up());
        Self
        {
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
        self.view = Mat4::look_to_lh(self.transform.position, self.transform.forward(), self.transform.up());
        self.view
    }
    pub fn update_projection(&mut self) -> Mat4
    {
        self.projection = Mat4::perspective_lh(self.fov.0, self.aspect_ratio, self.near_clip, self.far_clip);
        self.projection
    }
}

#[repr(packed)]
pub struct CameraUniform
{
    pub proj_view: Mat4,
}

impl From<&Camera> for CameraUniform
{
    fn from(camera: &Camera) -> Self
    {
        Self { proj_view: camera.projection() * camera.view() }
    }
}
