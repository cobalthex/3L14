// [[vk::binding(0, 2)]]
// cbuffer Material
// {
    Texture2D<float4> Tex;
    SamplerState Sampler;
// };

struct PixelInput
{
    float4 clip_position: SV_POSITION;
    float4 normal: NORMAL;
    float2 texcoord: TEXCOORD0;
    float4 color: COLOR0;
};

struct Light
{
    float3 position;
    float3 direction;
};
Light light =
{
    float3(0, 5, -5),
    float3(0, -0.707, 0.707),
};


float4 ps_main(PixelInput in_pixel) : SV_Target
{
    return float4(1, 1, 0, 1);
}