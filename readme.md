## Async trait experiments

This crate contains some experiments with alternative representations of
async trait objects.

The crate introduces `DynamicFuture` as a return type for async trait objects.
A `DynamicFuture` is a type-erased and dynamically dispatched `Future` object,
whose implementation is defined through a manual vtable (`DynamicFutureVtable`).

`DynamicFuture<'a, T>` can therefore be used as an alternative to
`Pin<Box<dyn Future<Output=T> + 'a>>` for the usage in async traits.

`DynamicFuture` allows for more options regarding the `Future`s backing storage.
`Pin<Box<dyn Future<Output=T> + 'a>>` always requires a fresh allocation on
the heap for each instance of ture `Future` - due to the usage of `Box`.

A `DynamicFuture` can be implemented in the same fashion. However that's not the
only way, and there exist alternatives:

One alternative which is explored in this crate is to reuse the memory which is
allocated for a `Future` between calls to async functions.

Typically callers of async functions do immediately `.await` the `Future`s
returned by those. They do not create multiple `Future`s using the same async
function. E.g. an async Stream which could be defined as

```rust
pub trait AsyncStream {
    fn next<'a>(&'a mut self) -> DynamicFuture<'a, Option<u32>>;
}
```

would typically be consumed using:

```rust
while let Some(element) = stream.next().await {

}
```

In this case the `Future` returned by `AsyncStream::next()` will always be
polled to completion and dropped before the next `Future` is allocated.
Iterating through the `Stream` requires a dynamic allocation for each element
yielded from the `Stream`.

This crate experiments with an allocator for `DynamicFuture`s whish is able to
reuse allocated memory between calls on the trait: The `RecyclableFutureAllocator`.

The `RecyclableFutureAllocator` is intended to be embedded into trait objects and
to be used for all allocations of `DynamicFuture`s. It will remember about the
memory allocated for a previous call to the same async method which yielded a
`DynamicFuture`. If this memory is now no longer in use - because the `Future` had
been dropped - the allocator can reuse the memory for the next call to the method.

Thereby `RecyclableFutureAllocator` enables async trait objects which require
only a single memory allocation for an arbitrary amount of calls to the same
async method as long as there are no concurrent calls to the same method -
opposed to 1 allocation per call.

The allocator can easily be embedded into an object and be used to allocate
returned `DynamicFuture`s:

```rust
pub struct AsyncStreamImpl {
    state: StreamState,
    next_recycler: RecyclableFutureAllocator,
}

impl AsyncStream for AsyncStreamImpl {
    fn next<'a>(&'a mut self) -> DynamicFuture<'a, Option<u32>> {
        self.next_recycler.allocate(async move {
            // The actual future implementation
        })
    }
}
```

The field could be hidden through macros for more convenient use.
E.g. a new version of [async-trait](https://docs.rs/async-trait/0.1.40/async_trait/)
could internally set up and use recyclers to lower the cost of trait-object
methods which are called more than once.
This would however require a change of the utilized return type from
`Pin<Box<Future>>` to `DynamicFuture`.

### Prior Art

- The reuse of memory allocations for `Future`s was championed by Stephen Toub
  for the .NET framework.
  [This article](https://devblogs.microsoft.com/dotnet/async-valuetask-pooling-in-net-5/)
  provides a bit of background information. The `RecyclableFutureAllocator`
  is based on the findings that `Future`s returned from one object are typically
  consumed before the next `Future` is generated.
- `DynamicFuture` uses an object representation which is similar to what Rust
  is already utilizing for
  [std::task::Waker](https://doc.rust-lang.org/std/task/struct.Waker.html).
  Both types delegate memory management to a vtable and are not opinionated
  about how their internal state is stored. 

### Is it worth it? / Benchmark results

The repository contains some benchmarks that can be run with

```
cargo bench --bench bench
```

which compare normal async methods vs
[async-trait](https://docs.rs/async-trait/0.1.40/async_trait/) vs traits using
`DynamicFuture` and `RecyclableFutureAllocator`.

The benchmarks are rather platform dependent, and fluctuate very strong with
the performance of the utilized memory allocator. On Windows, the performance
of async traits using the recycler is often 3x higher than those of async traits
using `Box<Pin<dyn Future>>` - if the method on the trait is called at least
2 times.

```
nested_stream_benches/no trait
                        time:   [89.082 ns 89.216 ns 89.380 ns]

nested_stream_benches/async-trait
                        time:   [5.2282 us 5.2461 us 5.2624 us]

nested_stream_benches/non recyclable DynamicFuture
                        time:   [3.2658 us 3.2727 us 3.2804 us]

nested_stream_benches/recyclable DynamicFuture
                        time:   [1.5016 us 1.5052 us 1.5099 us]
```

On Linux using jemalloc the performance using the recycler is still better - but
the gap is lower:

```
nested_stream_benches/no trait
                        time:   [89.635 ns 89.851 ns 90.093 ns]

nested_stream_benches/async-trait
                        time:   [2.3594 us 2.3711 us 2.3826 us]

nested_stream_benches/non recyclable DynamicFuture
                        time:   [1.6196 us 1.6252 us 1.6316 us]

nested_stream_benches/recyclable DynamicFuture
                        time:   [1.5435 us 1.5485 us 1.5531 us]
```

When using glibc malloc the performance using the recycler actually seems lower -
it seems like the glibc allocator might be very efficient with tiny short lived
allocations:

```
nested_stream_benches/no trait
                        time:   [89.435 ns 89.653 ns 89.867 ns]

nested_stream_benches/async-trait
                        time:   [1.3948 us 1.3965 us 1.3986 us]

nested_stream_benches/non recyclable DynamicFuture
                        time:   [1.1688 us 1.1816 us 1.1949 us]

nested_stream_benches/recyclable DynamicFuture
                        time:   [1.5075 us 1.5116 us 1.5158 us]
```

However all those benchmark results might not translate well to real application
results. In typical applications the allocator will see more churn through
different code paths, and the cost of allocations might be higher. All the
alloctions for `Future`s might also be longer lived.

Another thing to take into account is that using this technique the memory
allocated for trait objects will be higher when they are "idle" (not actively
polled). This does not seem an issue for trait objects which are always active -
e.g. I/O traits - it could however bloat the memory profile of applications when
utilized for other use-cases.

