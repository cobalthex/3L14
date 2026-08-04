#include "Colors.hlsli"
#include "Scene.hlsli"

struct VertexOutput
{
    float4 world_position: POSITION;
    float4 clip_position: SV_POSITION;
    float4 normal: NORMAL;
    float2 texcoord: TEXCOORD0;
};

VertexOutput vs_main(
    float3 in_position : POSITION,
    float3 in_normal : NORMAL,
    float3 in_texcoord : TEXCOORD0)
{
    VertexOutput out_vertex;
    out_vertex.world_position = mul(World, float4(in_position, 1));
    out_vertex.clip_position = mul(ProjView, out_vertex.world_position);
    out_vertex.normal = float4(in_normal, 1);
    out_vertex.texcoord = in_texcoord;
    return out_vertex;
}
