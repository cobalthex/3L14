fn lerp(from: f32, to: f32, rel: f32) -> f32
{
    ((1.0 - rel) * from) + (rel * to)
}

fn inv_lerp(from: f32, to: f32, value: f32) -> f32
{
    (value - from) / (to - from)
}

fn remap(orig_from: f32, orig_to: f32, target_from: f32, target_to: f32, value: f32) -> f32
{
    let rel = inv_lerp(orig_from, orig_to, value);
    lerp(target_from, target_to, rel)
}