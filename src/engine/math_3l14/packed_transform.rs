use std::fmt::{Debug, Formatter};
use bitcode::{Decode, Encode};
use glam::Quat;
use half::f16;
use nab_3l14::utils::ShortTypeName;
use crate::{DualQuat, NQuat48};

// A compressed dual quat for storing on-disk
#[derive(Default, PartialEq, Clone, Copy, Encode, Decode)]
pub struct PackedTransform
{
    real: NQuat48,
    dual: [u16; 4], // bit-cast half::f16 values
    // f16 unified scale?

    // TODO: reconstruct dual w from [ dual.w = -(real.x * dual.x + real.y * dual.y + real.z * dual.z) / real.w ] -- VERIFY MATH
    //       will need to handle conditions where real.w near 0 (dual quat mirror trick)
    // (or store sign separately)
    // could do
    // let inv_w = fast_reciprocal(real.w); // or load from LUT
    // dual.w = -(real.x * dx + real.y * dy + real.z * dz) * inv_w;

    // TODO: could also store transform as delta encoded for smaller size (e.g. 5 bits/component)
    // reconstruct full dual from translation is more ops, but can be SIMD'd (no division)

    // TODO: if quantizing translation, animation could store the quantization range for better precision
}
impl From<DualQuat> for PackedTransform
{
    fn from(value: DualQuat) -> Self
    {
        // assert normalized or just normalize?
        Self
        {
            real: value.real.into(),
            dual:
            [
                // TODO: SIMD
                // use custom bias and cast to integer?
                f16::from_f32(value.dual.x).to_bits(),
                f16::from_f32(value.dual.y).to_bits(),
                f16::from_f32(value.dual.z).to_bits(),
                f16::from_f32(value.dual.w).to_bits(),
            ]
        }
    }
}
impl From<PackedTransform> for DualQuat
{
    fn from(value: PackedTransform) -> Self
    {
        Self::from_raw(
            value.real.into(),
            Quat::from_xyzw(
                // TODO: SIMD
                f16::from_bits(value.dual[0]).to_f32(),
                f16::from_bits(value.dual[1]).to_f32(),
                f16::from_bits(value.dual[2]).to_f32(),
                f16::from_bits(value.dual[3]).to_f32(),
            )
        )
    }
}
impl Debug for PackedTransform
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        f.debug_struct(Self::short_type_name())
            .field("real", &self.real)
            .field("dual", &Quat::from_xyzw(
                // TODO: SIMD
                f16::from_bits(self.dual[0]).to_f32(),
                f16::from_bits(self.dual[1]).to_f32(),
                f16::from_bits(self.dual[2]).to_f32(),
                f16::from_bits(self.dual[3]).to_f32(),
            ))
            .finish()
    }
}

#[cfg(test)]
mod tests
{
    use approx::assert_relative_eq;
    use glam::Vec3;
    use super::*;

    #[test]
    fn to_from()
    {
        let dq = DualQuat::from_rot_trans(
            Quat::from_axis_angle(Vec3::new(1.0, 0.0, 0.0), 2.0).normalize(),
            Vec3::new(11.0, 22.0, 33.0),
        );

        let sm_dq =  PackedTransform::from(dq);
        let dq_expected = DualQuat::from(sm_dq);

        println!(" sm: {:?}", sm_dq);
        println!(" dq: {:?}\nexp: {:?}", dq, dq_expected);
        assert_relative_eq!(dq_expected, dq, epsilon = 1e-2); // note: certain rotations will be better represented than others
    }
}