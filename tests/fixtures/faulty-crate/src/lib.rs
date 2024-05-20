#![allow(internal_features)]
#![feature(abi_ptx, core_intrinsics)]
#![no_std]

#[no_mangle]
pub unsafe extern "ptx-kernel" fn the_kernel(x: *const f64, y: *mut f64, a: f64) {
    *y.offset(0) = external_fn(*x.offset(0)) * a;
}

#[panic_handler]
unsafe fn breakpoint_panic_handler(_: &::core::panic::PanicInfo) -> ! {
    core::intrinsics::breakpoint();
    core::hint::unreachable_unchecked();
}
