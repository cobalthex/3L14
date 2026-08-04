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
