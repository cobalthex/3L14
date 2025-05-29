#include "Colors.hlsli"
#include "math/DualQuat.hlsli"

[[vk::binding(0, 0)]]
cbuffer PerView_Camera
{
    float4x4 ProjView;
    uint TotalSecsWhole;
    float TotalSecsFrac;
};
[[vk::binding(0, 1)]]
cbuffer PerModel_World
{
    float4x4 World;
};

#define MAX_SKINNED_BONES 128

[[vk::binding(0, 2)]]
cbuffer PerModel_SkinnedPoses
{
    DualQuat SkinnedPoses[MAX_SKINNED_BONES];
}

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
    uint color : COLOR0,
    uint4 indices: BLENDINDICES,
    float4 weights: BLENDWEIGHT)
{
    VertexOutput out_vertex;

    DualQuat blended = DualQuatBlend4(
        SkinnedPoses[indices.x], weights.x,
        SkinnedPoses[indices.y], weights.y,
        SkinnedPoses[indices.z], weights.z,
        SkinnedPoses[indices.w], weights.w);
    float3 transformed_pos = DualQuatTransformPoint(blended, in_position);

    float3 transform_norm = DualQuatTransformDirection(blended, in_normal);
//     float4 transform_tan = float4(DualQuatTransformDirection(blended, in_tangent.xyz), in_tangent.w);

    out_vertex.world_position = mul(World, float4(transformed_pos, 1.0));
    out_vertex.clip_position = mul(ProjView, out_vertex.world_position);
    out_vertex.normal = float4(transform_norm, 0.0);
    out_vertex.texcoord = in_texcoord;
    out_vertex.color = UnpackRgba(color);
    return out_vertex;
}
