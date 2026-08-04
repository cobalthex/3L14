struct CameraUniform
{
    proj_view: mat4x4f,
    total_secs: f32,
};
struct WorldUniform
{
    transform: mat4x4f,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var<uniform> world: WorldUniform;

@group(2) @binding(0)
var tex: texture_2d<f32>;
@group(2) @binding(1)
var tex_sampler: sampler;

struct Light
{
    position: vec3f,
    direction: vec3f,
};
const light: Light = Light(
    vec3(0, 5, -5),
    vec3(0, -0.707, 0.707),
);

struct VertexOutput
{
    @builtin(position) clip_position: vec4f,
    @location(0) normal: vec3f,
    @location(1) texcoord: vec2f,
    @location(2) color: vec4f,
};

@vertex
fn vs_main(
    @location(0) in_position: vec3f,
    @location(1) in_normal: vec3f,
    @location(2) in_texcoord: vec2f,
    @location(3) in_color: u32
) -> VertexOutput
{
    var out_vertex: VertexOutput;
    out_vertex.clip_position = (camera.proj_view * world.transform) * vec4(in_position, 1.0);
    out_vertex.normal = (world.transform * vec4(in_normal, 1.0)).xyz;
    out_vertex.texcoord = in_texcoord;
    out_vertex.texcoord.y += camera.total_secs / 10.0;

    var r: f32 = f32((in_color >> 24) & 0xFFu) / 255.0;
    var g: f32 = f32((in_color >> 16) & 0xFFu) / 255.0;
    var b: f32 = f32((in_color >> 8) & 0xFFu) / 255.0;
    var a: f32 = f32(in_color & 0xFFu) / 255.0;
    out_vertex.color = vec4(r, g, b, a);

    return out_vertex;
}

@fragment
fn ps_main(in_frag: VertexOutput) -> @location(0) vec4f
{
    let light = max(dot(in_frag.normal.xyz, light.direction), 0.0);
    let tex_sample = textureSample(tex, tex_sampler, in_frag.texcoord);
    return tex_sample;// * light;
    //return in_frag.color * light;
}