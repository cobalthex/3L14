use crate::{debug_label, Renderer, Rgba};
use arrayvec::ArrayVec;
use bitcode::{Decode, Encode};
use proc_macros_3l14::FancyEnum;
use serde::{Deserialize, Serialize};
use std::error::Error;
use triomphe::Arc;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferSize, BufferUsages, SamplerBindingType, ShaderStages, TextureSampleType, TextureViewDimension};
use asset_3l14::{Ash, Asset, AssetKey, AssetLifecycler, AssetLoadRequest, AssetTypeId};
use debug_3l14::debug_gui::DebugGui;
use crate::assets::Texture;

pub const MAX_MATERIAL_TEXTURE_BINDINGS: usize = 16;

#[derive(PartialEq, Eq, Serialize, Deserialize, Encode, Decode, Debug, FancyEnum, Hash, Clone, Copy)]
pub enum MaterialClass
{
    SimpleOpaque,
}

#[repr(u64)]
pub enum MaterialFeatureFlags
{
    None = 0b0000000000000000,

}

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub struct PbrProps
{
    pub albedo_color: Rgba,
    pub metallicity: f32,
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct MaterialFile
{
    pub class: MaterialClass,
    pub textures: ArrayVec<AssetKey, MAX_MATERIAL_TEXTURE_BINDINGS>,
    pub props: Box<[u8]>,
}

pub struct Material
{
    pub class: MaterialClass,
    pub props: Buffer,
    pub bind_layout: BindGroupLayout,
    pub textures: ArrayVec<Ash<Texture>, MAX_MATERIAL_TEXTURE_BINDINGS>,
}
impl Asset for Material
{
    type DebugData = ();
    fn asset_type() -> AssetTypeId { AssetTypeId::Material }
    fn all_dependencies_loaded(&self) -> bool
    {
        self.textures.iter().all(|t| t.is_loaded_recursive())
    }
}

pub struct MaterialLifecycler
{
    renderer: Arc<Renderer>,
}
impl MaterialLifecycler
{
    pub fn new(renderer: Arc<Renderer>) -> Self
    {
        Self { renderer }
    }
}
impl AssetLifecycler for MaterialLifecycler
{
    type Asset = Material;

    fn load(&self, mut request: AssetLoadRequest) -> Result<Self::Asset, Box<dyn Error>>
    {
        let mtl_file: MaterialFile = request.deserialize()?;

        let textures: ArrayVec<Ash<Texture>, MAX_MATERIAL_TEXTURE_BINDINGS> = mtl_file.textures.iter().map(|t|
        {
           request.load_dependency(*t)
        }).collect();

        let props = self.renderer.device().create_buffer_init(&BufferInitDescriptor
        {
            label: debug_label!(&format!("{:?}", request.asset_key)),
            contents: &mtl_file.props,
            usage: BufferUsages::UNIFORM,
        });

        let mut layout_entries = Vec::new();

        match mtl_file.class
        {
            MaterialClass::SimpleOpaque =>
            {
                layout_entries.push(BindGroupLayoutEntry
                {
                    binding: layout_entries.len() as u32,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer
                    {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(props.size()),
                    },
                    count: None,
                });
            }
        }

        layout_entries.push(BindGroupLayoutEntry
        {
            binding: layout_entries.len() as u32,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Sampler(SamplerBindingType::Filtering),
            count: None,
        });

        layout_entries.push(BindGroupLayoutEntry
        {
            binding: layout_entries.len() as u32,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture
            {
                // TODO, this needs to come from the material class or the textures directly
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None, // TODO: this is probably easier
        });

        let bind_layout = self.renderer.device().create_bind_group_layout(&BindGroupLayoutDescriptor
        {
            label: debug_label!(&format!("{:?} layout", request.asset_key)),
            entries: &layout_entries,
        });

        Ok(Material
        {
            class: mtl_file.class,
            textures,
            props,
            bind_layout,
        })
    }
}
impl DebugGui for MaterialLifecycler
{
    fn display_name(&self) -> &str { "Materials" }
    fn debug_gui(&self, _ui: &mut egui::Ui) { }
}
