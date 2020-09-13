use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// A dynamically dispatched `Future`
///
/// The actual implementation is hidden behind the `Futures`s vtable.
/// The main requirement for such a `Future is that it's backing storage location
/// heap allocated and does not move while the `Future` is not dropped.
///
/// Thereby this `Future` can be `Unpin`
pub struct DynamicFuture<'a, T> {
    inner: *const (),
    /// The vtable which defines how the `Future` is polled and dropped.
    /// This should actually use a `&'static` lifetime - however for some reason
    /// the Rust compiler does not like that one.
    vtable: &'a DynamicFutureVtable<T>,
    /// Allows to store a lifetime with the `Future` if required
    _phantom: PhantomData<&'a ()>,
}

// This Future is always `Unpin`, since the actual future is stored on the heap
// and has a pinned location
impl<'a, T> Unpin for DynamicFuture<'a, T> {}

impl<'a, T> Drop for DynamicFuture<'a, T> {
    fn drop(&mut self) {
        // Delegate destruction of the `Future` to the vtable
        unsafe {
            (self.vtable.drop_fn)(self.inner as *const ());
        }
    }
}

impl<'a, T> Future for DynamicFuture<'a, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe { (self.vtable.poll_fn)(self.inner as *const (), cx) }
    }
}

impl<'a, T> DynamicFuture<'a, T> {
    /// Creates a new `DynamicFuture`.
    ///
    /// This method is `unsafe`. The caller must guarantee that the vtable and
    /// ptr are valid, and applying the methods of the vtable onto the pointer
    /// results in a correctly behaving and safe future implementation.
    pub unsafe fn new(ptr: *const (), vtable: &'a DynamicFutureVtable<T>) -> Self {
        Self {
            inner: ptr,
            vtable,
            _phantom: PhantomData,
        }
    }

    /// Returns the pointer stored in this `Future`
    pub fn ptr(&self) -> *const () {
        self.inner
    }

    /// Returns the vtable stored in this `Future`
    pub fn vtable(&self) -> &'a DynamicFutureVtable<T> {
        self.vtable
    }
}

/// Defines the behavior of a dynamically dispatched `Future`
pub struct DynamicFutureVtable<T> {
    /// Advances the state of this `Future`. This method is called every time
    /// the `Future` is `.poll()`d.
    pub poll_fn: unsafe fn(*const (), &mut Context<'_>) -> Poll<T>,
    /// Drops the `Future`.
    pub drop_fn: unsafe fn(*const ()),
}
