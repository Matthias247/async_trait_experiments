//! Stores a `Future` in a `Box` on the heap.
//!
//! However in comparison to `Pin<Box<dyn Future>>` this mechanism will retain
//! the `DynamicFuture` contract.

use crate::{DynamicFuture, DynamicFutureVtable};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

unsafe fn drop_boxed_future<F>(ptr: *const ()) {
    let fut: Box<F> = Box::from_raw(ptr as *const F as *mut F);
    drop(fut);
}

unsafe fn poll_boxed_future<T, F: Future<Output = T>>(
    ptr: *const (),
    cx: &mut Context<'_>,
) -> Poll<T> {
    let fut: &mut F = &mut *(ptr as *const F as *mut F);
    let pinned = Pin::new_unchecked(fut);
    pinned.poll(cx)
}

fn boxed_future_vtable<'a, F: Future<Output = T> + 'a, T>() -> &'a DynamicFutureVtable<T> {
    &DynamicFutureVtable {
        drop_fn: drop_boxed_future::<F>,
        poll_fn: poll_boxed_future::<T, F>,
    }
}

/// Stores a `Future` in a `Box` on the heap.
///
/// However in comparison to `Pin<Box<dyn Future>>` this mechanism will retain
/// the `DynamicFuture` contract.
pub fn box_future<'a, F, T>(fut: F) -> DynamicFuture<'a, T>
where
    F: Future<Output = T> + 'a,
{
    let b = Box::new(fut);
    unsafe { DynamicFuture::new(Box::into_raw(b) as *const (), boxed_future_vtable::<F, T>()) }
}
