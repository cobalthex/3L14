// use std::sync::Arc;
// use wgpu::{BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, SamplerBindingType, ShaderStages, TextureAspect, TextureSampleType, TextureViewDescriptor, TextureViewDimension};
// use crate::engine::assets::*;
// use crate::engine::graphics::assets::texture::Texture;
// use crate::engine::graphics::colors::Color;
// use crate::engine::graphics::{colors, Renderer};
//
// pub struct Material
// {
//     pub albedo_map: AssetHandle<Texture>,
//     pub albedo_color: Color,
//     pub metallicity: f32,
//     pub roughness: f32,
// }
// impl Asset for Material
// {
// }
//
// pub struct MaterialLifecycler;
// impl AssetLifecycler for MaterialLifecycler
// {
//     type Asset = Material;
//     fn load(&self, request: AssetLoadRequest) -> AssetPayload<Self::Asset>
//     {
//         todo!()
//     }
// }