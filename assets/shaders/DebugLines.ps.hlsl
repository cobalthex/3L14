#include "Colors.hlsli"

struct PixelInput
{
    float4 world_position: POSITION;
    float4 clip_position: SV_POSITION;
    float4 color: COLOR0;
};

float4 ps_main(PixelInput in_pixel) : SV_Target
{
    return in_pixel.color;
}