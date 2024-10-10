[[vk::binding(0, 0)]]
cbuffer Camera
{
    float4x4 ProjView;
    float TotalSecs;
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

float4 UnpackColor(uint packed)
{
    uint r = packed & 0xff;
    uint g = (packed >> 8) & 0xff;
    uint b = (packed >> 16) & 0xff;
    uint a = (packed >> 24) & 0xff;

    return float4(r, g, b, a) / 255.0;
}


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
    out_vertex.color = UnpackColor(color);
    return out_vertex;
}
