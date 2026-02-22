#include "Scene.hlsli"

float4 vs_main(float3 in_position : POSITION) : SV_POSITION
{
    float4 world_position = mul(World, float4(in_position, 1));
    return mul(ProjView, world_position);
}
