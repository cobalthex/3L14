struct CameraUniform
{
    proj_view: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexOutput
{
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) in_position: vec3<f32>,
    @location(1) in_normal: vec3<f32>,
    @location(2) in_texcoord: vec2<f32>,
    @location(3) in_color: u32
) -> VertexOutput
{
    var out_vertex: VertexOutput;
    out_vertex.clip_position = camera.proj_view * vec4(in_position, 1.0);

    var r: f32 = f32((in_color >> 24) & 0xFFu) / 255.0;
    var g: f32 = f32((in_color >> 16) & 0xFFu) / 255.0;
    var b: f32 = f32((in_color >> 8) & 0xFFu) / 255.0;
    var a: f32 = f32(in_color & 0xFFu) / 255.0;
    out_vertex.color = vec4(r, g, b, a);

    return out_vertex;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32>
{
    return in.color;
}