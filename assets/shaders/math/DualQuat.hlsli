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

float3 DualQuatTransformDirection(DualQuat dq, float3 direction)
{
    return direction + 2.0 * cross(dq.real.xyz, cross(dq.real.xyz, direction) + dq.real.w * direction);
}

float3 DualQuatTransformPoint(DualQuat dq, float3 position)
{
    float3 rotation = DualQuatTransformDirection(dq, position);
    float3 translation = 2.0 * (dq.real.w * dq.dual.xyz - dq.dual.w * dq.real.xyz + cross(dq.real.xyz, dq.dual.xyz));

    return rotation + translation;
}
