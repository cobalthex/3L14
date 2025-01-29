#include "Colors.hlsli"

struct VertexOutput
{
    float4 clip_position: SV_POSITION;
    float4 color: COLOR0;
};

VertexOutput vs_main(
    float4 in_position: POSITION,
    uint color: COLOR0)
{
    VertexOutput out_vertex;
    out_vertex.clip_position = in_position;
    out_vertex.color = UnpackColor(color);
    return out_vertex;
}
