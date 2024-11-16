static const uint MAX_INFLUENTIAL_LIGHTS = 8;

enum LightType : uint
{
    LightType_Point,
};

struct Light
{
    LightType type;
    float3 position;
    float3 direction;
    uint color_rgb;
};

struct Lights
{
    uint ambient_color_rgb;

    uint num_influential_lights;
    uint influential_lights[MAX_INFLUENTIAL_LIGHTS];
};

struct Env
{
    Lights lights;
};