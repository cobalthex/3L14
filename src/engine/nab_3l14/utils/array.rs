use std::mem::MaybeUninit;

// just use array-init crate?

pub fn init_array<T, const N: usize>(init_fn: impl Fn(usize) -> T) -> [T; N]
{
    let mut array = [const { MaybeUninit::uninit() }; N];
    for i in 0..N
    {
        array[i] = MaybeUninit::new(init_fn(i));
    }
    unsafe { array.as_ptr().cast::<[T; N]>().read() }
}