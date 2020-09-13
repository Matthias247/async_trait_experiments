use crate::{box_future, DynamicFuture, DynamicFutureVtable};
use std::{
    alloc::Layout,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};

/// An allocator for `DynamicFuture`s which can reuse storage.
///
/// If the future which was returned by this allocator had been polled to completion
/// and is dropped, the allocator can reuse the memory allocated for it to return
/// another future of the same type.
pub struct RecyclableFutureAllocator {
    recycled: *const RecyclableFutureHeader,
}

impl Default for RecyclableFutureAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for RecyclableFutureAllocator {
    fn drop(&mut self) {
        if !self.recycled.is_null() {
            unsafe {
                // Decrement the refcount
                if (*self.recycled).refcount.fetch_sub(1, Ordering::Relaxed) == 1 {
                    // Free the memory allocated for the recyclable future
                    (*(self.recycled as *mut RecyclableFutureHeader)).deallocate();
                }
            }
        }
    }
}

impl RecyclableFutureAllocator {
    pub fn new() -> Self {
        Self {
            recycled: std::ptr::null(),
        }
    }

    /// Transforms the passed future into a `DynamicFuture`.
    ///
    /// This action will move the future on the heap and type erase its behavior.
    /// The operation will reuse memory from a previous `allocate` call if possible.
    pub fn allocate<'a, F, T>(&mut self, fut: F) -> DynamicFuture<'a, T>
    where
        F: Future<Output = T> + 'a,
    {
        unsafe {
            if self.recycled.is_null() {
                // Since we retain a reference to this future it needs to have
                // a refcount of 2
                let fut = new_recyclable_future(fut, 2);
                self.recycled = fut.ptr() as *const RecyclableFutureHeader;
                return fut;
            }

            // Check whether the layout is compatible with the layout of the
            // backing storage.
            // We don't worry about the alignment - since the alignment of the
            // header should fit everything else.
            if (*self.recycled).size != Layout::for_value(&fut).size() {
                return box_future(fut);
            }

            // If the current futures storage is no longer in use we can reuse
            // it for the next future.
            // Otherwise we will allocate a detached future
            match (*self.recycled).refcount.compare_exchange(
                1,
                2,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    std::ptr::write((*self.recycled).payload_addr_mut(), fut);
                    let header = self.recycled;
                    DynamicFuture::new(header as *const (), recyclable_future_vtable::<F, T>())
                }
                Err(2) => {
                    // The future is still in use.
                    // Allocate a fresh future
                    box_future(fut)
                }
                Err(refcount) => panic!("Invalid future refcount of {}", refcount),
            }
        }
    }
}

unsafe fn drop_recyclable_future<F>(ptr: *const ()) {
    let header = ptr as *const RecyclableFutureHeader;
    // Call the `drop` on the `Future` stored inside the header
    let data: *mut F = (*header).payload_addr_mut::<F>();
    std::ptr::drop_in_place(data);

    // Decrement the refcount and free storage if not utilized anymore
    if (*header).refcount.fetch_sub(1, Ordering::Release) == 1 {
        // Deallocate header and storage
        (*(header as *mut RecyclableFutureHeader)).deallocate();
    }
}

unsafe fn poll_recyclable_future<T, F: Future<Output = T>>(
    ptr: *const (),
    cx: &mut Context<'_>,
) -> Poll<T> {
    let header = ptr as *const RecyclableFutureHeader;
    let fut: &mut F = &mut *((*header).payload_addr_mut::<F>());
    let pinned = Pin::new_unchecked(fut);
    pinned.poll(cx)
}

fn recyclable_future_vtable<'a, F: Future<Output = T> + 'a, T>() -> &'a DynamicFutureVtable<T> {
    &DynamicFutureVtable {
        drop_fn: drop_recyclable_future::<F>,
        poll_fn: poll_recyclable_future::<T, F>,
    }
}

/// Creates a fresh recyclable future by allocating storage for it on the heap
pub fn new_recyclable_future<'a, F, T>(fut: F, initial_refcount: usize) -> DynamicFuture<'a, T>
where
    F: Future<Output = T> + 'a,
{
    unsafe {
        let header =
            RecyclableFutureHeader::allocate(Layout::for_value(&fut), initial_refcount).unwrap();
        std::ptr::write((*header).payload_addr_mut(), fut);
        DynamicFuture::new(header as *const (), recyclable_future_vtable::<F, T>())
    }
}

/// A header stored in front of recyclable `Future`s on the heap.
///
/// The location of a heap allocated Future can be determined by the location
/// of its header.
#[derive(Debug)]
struct RecyclableFutureHeader {
    /// The amount of active references to this memory location.
    /// Only up to 2 references can exist:
    /// 1. The reference from the `Future`
    /// 2. The reference from the `RecyclableFutureAllocator`
    refcount: AtomicUsize,
    /// The size of the `Future` which is stored behind the header according
    /// to its `Layout`
    size: usize,
}

impl RecyclableFutureHeader {
    /// Allocates space for a `RecyclableFutureHeader` and a payload which requires
    /// the space of `data_layout` on the heap.
    unsafe fn allocate(
        data_layout: Layout,
        initial_refcount: usize,
    ) -> Result<*mut RecyclableFutureHeader, ()> {
        // We shouldn't have any alignment issues, since `RecyclableFutureHeader`
        // is aligned to `usize` - which should cover what everything else needs.
        // But let's do a debug check.
        // Not having to store the alignment will save 8 bytes here.
        debug_assert!(
            Layout::new::<RecyclableFutureHeader>().align() >= data_layout.align()
                && Layout::new::<RecyclableFutureHeader>().align() % data_layout.align() == 0
        );

        let combined_layout = RecyclableFutureHeader::layout_for_size(data_layout.size())?;
        let alloc_res = std::alloc::alloc(combined_layout) as *mut RecyclableFutureHeader;
        if alloc_res.is_null() {
            return Err(());
        }

        let result = &mut *alloc_res;
        // Storing the initial refcount is not required to be atomic since
        // the value is not visible to other threads at this time.
        result.refcount = AtomicUsize::new(initial_refcount);
        result.size = data_layout.size();

        Ok(alloc_res)
    }

    fn layout_for_size(data_size: usize) -> Result<Layout, ()> {
        let layout = Layout::new::<RecyclableFutureHeader>();
        let total_size = layout.size().checked_add(data_size).ok_or(())?;
        let combined_layout = Layout::from_size_align(total_size, layout.align()).map_err(|e| {
            eprintln!("Layout error: {}", e);
            ()
        })?;
        Ok(combined_layout)
    }

    unsafe fn deallocate(&mut self) {
        if let Ok(layout) = RecyclableFutureHeader::layout_for_size(self.size) {
            std::alloc::dealloc(self as *mut RecyclableFutureHeader as *mut u8, layout);
        }
    }

    /// Returns the address of the payload section which is allocated behind
    /// the header.
    unsafe fn payload_addr<T>(&self) -> *const T {
        let mut end_addr = self as *const RecyclableFutureHeader as usize;
        end_addr += std::mem::size_of::<RecyclableFutureHeader>();
        end_addr as *const T
    }

    /// Returns the address of the payload section which is allocated behind
    /// the header.
    unsafe fn payload_addr_mut<T>(&self) -> *mut T {
        self.payload_addr::<T>() as *mut T
    }
}
