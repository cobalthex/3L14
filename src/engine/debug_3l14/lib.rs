pub mod debug_gui;
pub mod debug_menu;
pub mod sparkline;

// insert a debug breakpoint at this line
#[inline(always)]
pub fn debug_breakpoint()
{
    #[cfg(debug_assertions)]
    unsafe
    {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        core::arch::asm!("int3", options(nomem, nostack, preserves_flags));
        #[cfg(target_arch = "aarch64")]
        core::arch::asm!("brk #0xf000", options(nomem, nostack));
    }
}