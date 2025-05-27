use crate::assets::{Geometry, Material, Shader, Skeleton};
use crate::Renderer;
use asset_3l14::{Asset, Ash, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use bitcode::{Decode, Encode};
use debug_3l14::debug_gui::DebugGui;
use std::error::Error;
use std::sync::Arc;

#[derive(Encode, Decode)]
pub struct ModelFileSurface
{
    pub material: AssetKey,
    pub vertex_shader: AssetKey,
    pub pixel_shader: AssetKey,
}

#[derive(Encode, Decode)]
pub struct ModelFile
{
    pub geometry: AssetKey,
    pub skeleton: Option<AssetKey>,
    pub surfaces: Box<[ModelFileSurface]>,
}

pub struct Surface
{
    pub material: Ash<Material>,
    pub vertex_shader: Ash<Shader>,
    pub pixel_shader: Ash<Shader>,
}

pub struct Model
{
    pub mesh_count: u32,
    pub geometry: Ash<Geometry>,
    pub skeleton: Option<Ash<Skeleton>>,
    pub surfaces: Box<[Surface]>,
}
impl Asset for Model
{
    type DebugData = ();
    fn asset_type() -> AssetTypeId { AssetTypeId::Model }
    fn all_dependencies_loaded(&self) -> bool
    {
        self.geometry.is_loaded_recursive() &&
        self.skeleton.as_ref().map_or(true, |s| s.is_loaded_recursive()) &&
        self.surfaces.iter().all(|s|
            {
                s.material.is_loaded_recursive() &&
                s.vertex_shader.is_loaded_recursive() &&
                s.pixel_shader.is_loaded_recursive()
            })
    }
}

pub struct ModelLifecycler
{
    renderer: Arc<Renderer>,
}
impl ModelLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }
}
impl AssetLifecycler for ModelLifecycler
{
    type Asset = Model;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let model_file: ModelFile = request.deserialize()?;

        Ok(Model
        {
            mesh_count: model_file.surfaces.len() as u32, // store explicitly in file?
            geometry: request.load_dependency(model_file.geometry),
            skeleton: model_file.skeleton.map(|s| request.load_dependency(s)),
            surfaces: model_file.surfaces.iter().map(|s|
            {
                Surface
                {
                    material: request.load_dependency(s.material),
                    vertex_shader: request.load_dependency(s.vertex_shader),
                    pixel_shader: request.load_dependency(s.pixel_shader),
                }
            }).collect(),
        })
    }
}
impl DebugGui for ModelLifecycler
{
    fn display_name(&self) -> &str { "Models" }
    fn debug_gui(&self, ui: &mut egui::Ui) { }
}