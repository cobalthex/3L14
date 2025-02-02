#include "Colors.hlsli"

[[vk::binding(0, 0)]]
cbuffer Camera
{
    float4x4 ProjView;
    uint TotalSecsWhole;
    float TotalSecsFrac;
};
[[vk::binding(0, 1)]]
cbuffer World
{
    float4x4 World;
};

struct VertexOutput
{
    float4 world_position: POSITION;
    float4 clip_position: SV_POSITION;
    float4 normal: NORMAL;
    float2 texcoord: TEXCOORD0;
    float4 color: COLOR0;
};

VertexOutput vs_main(
    float3 in_position : POSITION,
    float3 in_normal : NORMAL,
    float3 in_texcoord : TEXCOORD0,
    uint color : COLOR0)
{
    VertexOutput out_vertex;
    out_vertex.world_position = mul(World, float4(in_position, 1));
    out_vertex.clip_position = mul(ProjView, out_vertex.world_position);
    out_vertex.normal = float4(in_normal, 1);
    out_vertex.texcoord = in_texcoord;
    out_vertex.color = UnpackRgba(color);
    return out_vertex;
}
