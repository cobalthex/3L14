// [[vk::binding(0, 2)]]
// Texture2D<float4> Tex;
// [[vk::binding(1, 2)]]
// SamplerState Sampler;

struct PixelInput
{
    float4 world_position: POSITION;
    float4 clip_position: SV_POSITION;
    float4 normal: NORMAL;
    float2 texcoord: TEXCOORD0;
    float4 color: COLOR0;
};

struct Light
{
    float3 position;
    float3 direction;
    float3 color;
};

static const float PI = 3.14159265359;
float4 ps_main(PixelInput in_pixel) : SV_Target
{
    Light light =
    {
        float3(0, 5, -5),
        float3(0.2673, 0.5345, 0.8018),
        float3(1, 1, 1),
    };
    float3 ambient = 0.03;

    float3 albedo = in_pixel.color.rgb;
    float metallicity = 0.5;
    float roughness = 0.2;
    float ao = 0.5;

    float lightDistSq = dot(light.position, in_pixel.world_position);
    float attenuation = 1 / lightDistSq; // rcp?
    float3 radiance = light.color * attenuation;

    float NdotL = max(dot(in_pixel.normal.xyz, light.direction.xyz), 0.0);

    float kD = 0.5; // TODO
    float specular = 0;

    float3 Lo = 0.1;
    Lo += (kD * albedo / PI + specular) * radiance * NdotL;

    //float4 texcol = Tex.Sample(Sampler, in_pixel.texcoord);
    float4 texcol = 1;
    return texcol * float4(Lo, 1) + float4(ambient * albedo * ao, 0);
}