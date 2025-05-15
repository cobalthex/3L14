struct DualQuat
{
    nointerpolation float4 real;
    nointerpolation float4 dual;
};

// simplified normalization
DualQuat DualQuatNormalize(DualQuat dq)
{
    float len = length(dq.real);
    dq.real /= len;
    dq.dual /= len;
    return dq;
}

DualQuat DualQuatBlend4(
    DualQuat dq0, float w0,
    DualQuat dq1, float w1,
    DualQuat dq2, float w2,
    DualQuat dq3, float w3)
{
    DualQuat result;
    result.real = (dq0.real * w0) + (dq1.real * w1) + (dq2.real * w2) + (dq3.real * w3);
    result.dual = (dq0.dual * w0) + (dq1.dual * w1) + (dq2.dual * w2) + (dq3.dual * w3);
    return DualQuatNormalize(result);
}

float3 DualQuatTransformPoint(DualQuat dq, float3 position)
{
    float4 real = dq.real;

    // translate [ 2 * (d * conjugate(r)).xyz ]
    float4 realConj = float4(-real.xyz, real.w);
    float4 transQuat = 2.0 * mul(dq.dual, realConj);

    // rotate
    float3 t = 2.0 * cross(real.xyz, position);
    float3 rotated = position + real.w * t + cross(real.xyz, t);

    return rotated + transQuat.xyz;
}

float3 DualQuatTransformDirection(DualQuat dq, float3 direction)
{
    // does quaternion rotation with the real part of the DQ [ q * v * q^-1 ]
    float3 t = 2.0 * cross(dq.real.xyz, direction);
    return normalize(direction + dq.real.w * t + cross(dq.real.xyz, t));
}