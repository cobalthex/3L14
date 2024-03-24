struct CameraUniform
{
    Mat4x4 proj_view;
    float total_secs;
};
struct WorldUniform
{
    Mat4x4 world;
};

cbuffer CameraUniform camera;
cbuffer WorldUniform world;

struct VertexOutput
{
    float4 clip_position: POSITION;
    float4 color: COLOR0;
}

VertexOutput vs_main(float3 in_position : POSITION, float3 in_normal : NORMAL, float3 in_texcoord : TEXCOORD0, float4 color : COLOR0)
{
    VertexOutput out_vertex;
    out_vertex.clip_position = (camera.proj_view * world.transform) * float4(in_position, 1);
    out_vertex.color = color;
}

float4 ps_main(VertexOutput in_vertex) : SV_Target
{
    return in_vertex.color;
}