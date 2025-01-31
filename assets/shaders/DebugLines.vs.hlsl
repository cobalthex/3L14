#include "Colors.hlsli"

struct Vertex
{
    float4 position;
    uint color;
};
StructuredBuffer<Vertex> InVertices : register(t0);

struct VertexOutput
{
    float4 clip_position: SV_POSITION;
    float4 color: COLOR0;
};

VertexOutput vs_main(uint in_index: SV_VertexID)
{
    Vertex in_vertex = InVertices[in_index];

    VertexOutput out_vertex;
    out_vertex.clip_position = in_vertex.position;
    out_vertex.color = UnpackColor(in_vertex.color);
    return out_vertex;
}
