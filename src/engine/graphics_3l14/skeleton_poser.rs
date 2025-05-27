use arrayvec::ArrayVec;
use math_3l14::{Affine3, DualQuat};
use nab_3l14::debug_panic;
use nab_3l14::timing::FSeconds;
use crate::assets::{AnimFrameNumber, SkeletalAnimation, Skeleton, MAX_SKINNED_BONES};

type PoseSet = ArrayVec<DualQuat, MAX_SKINNED_BONES>;

// rigger?
pub struct SkeletonPoser<'s>
{
    poses: PoseSet, // extenral memory (and limit to # of bones)?
    skeleton: &'s Skeleton,
}
impl<'s> SkeletonPoser<'s>
{
    pub fn new(skeleton: &'s Skeleton) -> Self
    {
        // todo: check on asset load
        debug_assert_eq!(skeleton.bone_ids.len(), skeleton.inv_bind_poses.len());
        debug_assert_eq!(skeleton.bone_ids.len(), skeleton.inv_bind_poses.len());
        debug_assert_eq!(skeleton.bone_ids.len(), skeleton.parent_indices.len());

        let num_poses = skeleton.bind_poses.len().min(MAX_SKINNED_BONES);
        let mut poses = ArrayVec::new();
        unsafe { poses.set_len(num_poses); } // better way?
        poses[..num_poses].copy_from_slice(&skeleton.bind_poses[..num_poses]);

        Self
        {
            poses,
            skeleton,
        }
    }

    pub fn build(mut self) -> PoseSet
    {
        // apply hierarchy
        for i in 0..self.poses.len()
        {
            let parent = self.skeleton.parent_indices[i];
            if parent >= 0
            {
                self.poses[i] = self.poses[parent as usize] * self.poses[i];
            }
        }

        // apply inv bind poses
        for i in 0..self.poses.len()
        {
            self.poses[i] = self.skeleton.inv_bind_poses[i] * self.poses[i];
        }

        self.poses
    }

    pub fn blend(&mut self, animation: &SkeletalAnimation, frame: AnimFrameNumber)
    {
        // TODO: inter-frame blending

        // TODO: blend mode (additive, replace, exlusive(?))
        if frame > animation.frame_count
        {
            debug_panic!("Frame {} is out of bounds of animation with length {}", frame.0, animation.frame_count.0);
            return;
        }

        for (i, pose) in animation.get_pose_for_frame(AnimFrameNumber(frame.0 as u32)).into_iter().enumerate()
        {
            let Some(bone_idx) = self.skeleton.bone_ids.iter().position(|b| *b == animation.bones[i])
                // else { panic!("Did not find matching bone for {:?} (#{i}) in skel:{:?}", anim.bones[i], skel.hierarchy); };
            else { continue; }; // skip; TODO this is likely happening when skin.skeleton node is animated
            self.poses[bone_idx] = *pose;
        }
    }
}

#[cfg(test)]
mod tests
{
    use std::f32::consts::FRAC_PI_2;
    use glam::{Quat, Vec3};
    use crate::assets::BoneId;
    use super::*;

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
            inv_bind_poses: Box::new([
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
        let posed = poser.build();
        assert_eq!(posed.len(), 3);

        println!("posed:");
        for p in posed.iter() { println!("  {p}") };
        println!("\nbind:");
        for p in skeleton.bind_poses.iter() { println!("  {p}") };
        println!("\ninv bind:");
        for p in skeleton.inv_bind_poses.iter() { println!("  {p}") };

        println!("\npose cancel:");
        (0..3)
            .map(|i| skeleton.inv_bind_poses[i] * skeleton.bind_poses[i])
            .for_each(|p| println!("  {} -- {:?}", p, p));

        assert_eq!(posed[0], DualQuat::IDENTITY);
        assert_eq!(posed[1], DualQuat::from_rot_trans(Quat::IDENTITY, Vec3::new(0.0, 2.0, 0.0)));
        assert_eq!(posed[2], DualQuat::from_rot_trans(Quat::from_axis_angle(Vec3::Z, FRAC_PI_2), Vec3::new(1.0, 2.0, 0.0)));
    }
}