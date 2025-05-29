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
    DualQuat dq0, float weight0,
    DualQuat dq1, float weight1,
    DualQuat dq2, float weight2,
    DualQuat dq3, float weight3)
{
    // Antipodal correction
    if (dot(dq0.real, dq1.real) < 0.0) { dq1.real *= -1.0; dq1.dual *= -1.0; }
    if (dot(dq0.real, dq2.real) < 0.0) { dq2.real *= -1.0; dq2.dual *= -1.0; }
    if (dot(dq0.real, dq3.real) < 0.0) { dq3.real *= -1.0; dq3.dual *= -1.0; }

    DualQuat result;
    result.real = (dq0.real * weight0) + (dq1.real * weight1) + (dq2.real * weight2) + (dq3.real * weight3);
    result.dual = (dq0.dual * weight0) + (dq1.dual * weight1) + (dq2.dual * weight2) + (dq3.dual * weight3);
    return DualQuatNormalize(result);
}

float3 DualQuatTransformPoint(DualQuat dq, float3 position)
{
    float4 real = dq.real;
    float4 dual = dq.dual;

    float3 pos = position + (2.0 * cross(real.xyz, cross(real.xyz, position) + (real.w * position)));
    float3 trans = 2.0 * (real.w * dual.xyz - dual.w * real.xyz + cross(real.xyz, dual.xyz));
    pos += trans;

    return pos;

    // // translate [ 2 * (d * conjugate(r)).xyz ]
    // float4 realConj = float4(-real.xyz, real.w);
    // float4 transQuat = 2.0 * mul(dq.dual, realConj);

    // // rotate
    // float3 t = 2.0 * cross(real.xyz, position);
    // float3 rotated = position + real.w * t + cross(real.xyz, t);

    // return rotated + transQuat.xyz;
}

float3 DualQuatTransformDirection(DualQuat dq, float3 direction)
{
    // does quaternion rotation with the real part of the DQ [ q * v * q^-1 ]
    float3 t = 2.0 * cross(dq.real.xyz, direction);
    return normalize(direction + dq.real.w * t + cross(dq.real.xyz, t));
}