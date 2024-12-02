#include "Colors.hlsli"
#include "Lighting.hlsli"

[[vk::binding(0, 2)]]
cbuffer Pbr
{
    uint albedo_color;
    float metallicity;
    float roughness;
};

[[vk::binding(1, 2)]]
SamplerState Sampler;
[[vk::binding(2, 2)]]
Texture2D<float4> Tex;

struct PixelInput
{
    float4 world_position: POSITION;
    float4 clip_position: SV_POSITION;
    float4 normal: NORMAL;
    float2 tex_coord: TEXCOORD0;
    float4 color: COLOR0;
};

static const float PI = 3.14159265359;
float4 ps_main(PixelInput in_pixel) : SV_Target
{
    Light light =
    {
        LightType_Point,
        float3(0, 5, -5),
        float3(0.2673, 0.5345, 0.8018),
        PackColor(float4(1, 1, 1, 1)),
    };
    float3 ambient = 0.03;

    float3 albedo = in_pixel.color.rgb;
    float metallicity = 0.5;
    float roughness = 0.2;
    float ao = 0.5;

    float light_dist_sq = dot(light.position, in_pixel.world_position.xyz);
    float attenuation = 1 / light_dist_sq; // rcp?
    float3 radiance = light.color_rgb * attenuation;

    float n_dot_l = max(dot(in_pixel.normal.xyz, light.direction), 0.0);

    float kD = 0.5; // TODO
    float specular = 0;

    float3 Lo = 0.1;
    Lo += (kD * albedo / PI + specular) * radiance * n_dot_l;

    float4 tex_col = Tex.Sample(Sampler, in_pixel.tex_coord);
    return tex_col * float4(Lo, 1) + float4(ambient * albedo * ao, 0);
}