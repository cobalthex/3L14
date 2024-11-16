float4 UnpackColor(uint packed) // assumes R = low bits
{
    uint r = packed & 0xff;
    uint g = (packed >> 8) & 0xff;
    uint b = (packed >> 16) & 0xff;
    uint a = (packed >> 24) & 0xff;

    return float4(r, g, b, a) / 255.0;
}

uint PackColor(float4 unpacked) // assumes R = low bits
{
    return
        ((int)(unpacked.r * 255.0) >> 0) |
        ((int)(unpacked.g * 255.0) >> 8) |
        ((int)(unpacked.b * 255.0) >> 16) |
        ((int)(unpacked.a * 255.0) >> 24);
}