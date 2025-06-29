use arrayvec::ArrayVec;
use math_3l14::{Affine3, DualQuat, Ratio};
use nab_3l14::{debug_panic, TickCount};
use nab_3l14::timing::FSeconds;
use crate::assets::{AnimFrameNumber, SkeletalAnimation, Skeleton, MAX_SKINNED_BONES};

type PoseSet = ArrayVec<DualQuat, MAX_SKINNED_BONES>;

pub enum PoseBlendMode
{
    Replace,
    Additive(f32),
    Exclusive,
}

// TODO: https://rodolphe-vaillant.fr/entry/72/bulge-free-dual-quaternion-skinning-trick
// TODO: https://rodolphe-vaillant.fr/entry/78/dual-quaternion-skinning-with-scale

// rigger?
pub struct SkeletonPoser<'s>
{
    poses: PoseSet, // extenral memory (and limit to # of bones)?
    skeleton: &'s Skeleton,
}
impl<'s> SkeletonPoser<'s>
{
    #[must_use]
    pub fn new(skeleton: &'s Skeleton) -> Self
    {
        // todo: check on asset load
        debug_assert_eq!(skeleton.bone_ids.len(), skeleton.inverse_bind_poses.len());
        debug_assert_eq!(skeleton.bone_ids.len(), skeleton.inverse_bind_poses.len());
        debug_assert_eq!(skeleton.bone_ids.len(), skeleton.parent_indices.len());

        let num_poses = skeleton.bind_poses.len().min(MAX_SKINNED_BONES);
        let mut poses = ArrayVec::new();
        unsafe { poses.set_len(num_poses); } // better way?
        // TODO: this should either be explicit or fill in gaps left by animation
        poses[..num_poses].copy_from_slice(&skeleton.bind_poses[..num_poses]);

        Self
        {
            poses,
            skeleton,
        }
    }

    // TODO: blend_no_lerp() ?

    // Apply an animation to the pose.
    pub fn blend(&mut self, animation: &SkeletalAnimation, mode: PoseBlendMode, time: TickCount, should_loop: bool)
    {
        puffin::profile_function!();
        
        // TODO: blend mode (additive, replace, exlusive(?))

        // would this be faster to convert to float first? (floating point div may be faster)
        // blend multiple animations at once w/ simd?

        let sample_rate = Ratio
        {
            numerator: animation.sample_rate.numerator as u64,
            denominator: animation.sample_rate.denominator as u64 * 1_000_000, // shouldn't this be 1e9?
        };

        let unclamped = (sample_rate.scale(time.0)) as u32;
        let (curr_frame, next_frame) = if should_loop
        {
            (unclamped % animation.frame_count.0, (unclamped + 1) % animation.frame_count.0)
        }
        else
        {
            (unclamped.min(animation.frame_count.0), (unclamped + 1).min(animation.frame_count.0))
        };

        let fraction =
        {
            let delta = time.0 - sample_rate.inverse_scale(unclamped as u64);
            (delta as f32) * sample_rate.to_f32()
        };

        let from = animation.get_pose_for_frame(AnimFrameNumber(curr_frame)).iter();
        let to = animation.get_pose_for_frame(AnimFrameNumber(next_frame)).iter();

        for (i, (fp, tp)) in from.zip(to).enumerate()
        {
            let bone_id = animation.bones[i];
            // TODO: animation should store bone indices
            let Some(bone_idx) = self.skeleton.bone_ids.iter().position(|b| *b == animation.bones[i])
                // else { panic!("Did not find matching bone for {:?} (#{i}) in skel:{:?}", anim.bones[i], skel.hierarchy); };
                else { continue; }; // skip; TODO this is likely happening when skin.skeleton node is animated

            // TODO: cache reconstructed dual-quats
            let lerped = DualQuat::nlerp(fp.clone().into(), tp.clone().into(), fraction);
            // TODO: move the branch out of the loop
            self.poses[bone_idx] = match mode
            {
                PoseBlendMode::Replace => lerped,
                PoseBlendMode::Additive(frac) => self.poses[bone_idx].nlerp(lerped, frac), // is this correct?
                PoseBlendMode::Exclusive => todo!("Exclusive skeletal animation blending"),
            }
        }
    }

    // Compute (and optionally returns) the skin/skeleton space poses (useful for drawing the skeleton)
    pub fn build_poses(&mut self) -> &[DualQuat]
    {
        // local to bone space
        for i in 0..self.poses.len()
        {
            let parent = self.skeleton.parent_indices[i];
            if parent >= 0
            {
                self.poses[i] = self.poses[parent as usize] * self.poses[i];
            }
        }

        &self.poses
    }

    // Transform the poses into model space and return the final poses
    // build_world_space() must be called first
    #[must_use]
    pub fn finalize(mut self) -> PoseSet
    {
        // bone to model space
        for i in 0..self.poses.len()
        {
            self.poses[i] = self.poses[i] * self.skeleton.inverse_bind_poses[i];
        }

        self.poses
    }
}

#[cfg(test)]
mod tests
{
    use std::f32::consts::FRAC_PI_2;
    use glam::{Quat, Vec3};
    use crate::assets::BoneId;
    use super::*;

    // TODO: fix
    fn generate_skeleton() -> Skeleton
    {
        let j0 = DualQuat::IDENTITY;
        let j1 = j0 * DualQuat::from_rot_trans(Quat::from_rotation_z(FRAC_PI_2), Vec3::new(0.0, 2.0, 0.0));
        let j2 = j1 * DualQuat::from_rot_trans(Quat::IDENTITY, Vec3::new(0.0, 1.0, 0.0));

        Skeleton
        {
            bone_ids: Box::new([
                BoneId(0),
                BoneId(1),
                BoneId(2)
            ]),
            parent_indices: Box::new([
                -1,
                0,
                1,
            ]),
            bind_poses: Box::new([
                j0,
                j1,
                j2,
            ]),
            inverse_bind_poses: Box::new([
                j0.inverse(),
                j1.inverse(),
                j2.inverse(),
            ])
        }
    }

    #[test]
    pub fn no_anim()
    {
        let skeleton = generate_skeleton();
        let poser = SkeletonPoser::new(&skeleton);
        let posed = poser.finalize();
        assert_eq!(posed.len(), 3);

        // TODO
    }
}