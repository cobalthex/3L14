struct VertexOutput
{
    float4 clip_position: POSITION;
    float4 color: COLOR0;
}

float4 ps_main(VertexOutput in_vertex) : SV_Target
{
    return in_vertex.color;
}