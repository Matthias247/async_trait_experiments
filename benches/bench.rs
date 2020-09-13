//! Benchmarks for asynchronous Mutex implementations

#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use criterion::{criterion_group, criterion_main, Benchmark, Criterion};
mod adder;
use adder::{
    AsyncTraitAdder, AsyncTraitAdderImpl, BoxPinFutureTraitAdder, BoxPinFutureTraitAdderImpl,
    DynamicFutureAsyncTraitAdder, DynamicRecyclableFutureAsyncTraitAdderImpl, NoTraitAdder,
};
mod stream;
use stream::{
    AsyncTraitStream, AsyncTraitStreamImpl, AsyncTraitWrappingStreamImpl,
    DynamicFutureAsyncTraitStream, DynamicRecyclableFutureAsyncTraitStreamImpl,
    DynamicRecyclableFutureAsyncTraitWrappingStreamImpl, NoTraitStream, NoTraitWrappingStream,
};

const ADDER_ITERATIONS: usize = 50;
const STREAM_ITERATIONS: u32 = 50;

fn adder_benches(c: &mut Criterion) {
    c.bench(
        "adder benches",
        Benchmark::new("no trait", |b| {
            b.iter(|| {
                let mut adder = NoTraitAdder::default();
                futures::executor::block_on(async {
                    for _ in 0..ADDER_ITERATIONS {
                        assert_eq!(25, adder.add_obj(5, 20).await);
                        assert_eq!(25, adder.current);
                    }
                });
            });
        })
        .with_function("async trait obj", |b| {
            b.iter(|| {
                futures::executor::block_on(async {
                    let mut adder = AsyncTraitAdderImpl::default();
                    for _ in 0..ADDER_ITERATIONS {
                        assert_eq!(25, adder.add_obj(5, 20).await);
                        assert_eq!(25, adder.current);
                    }
                });
            });
        })
        .with_function("box pin future trait obj", |b| {
            b.iter(|| {
                futures::executor::block_on(async {
                    let mut adder = BoxPinFutureTraitAdderImpl::default();
                    for _ in 0..ADDER_ITERATIONS {
                        assert_eq!(25, adder.add_obj(5, 20).await);
                        assert_eq!(25, adder.current);
                    }
                });
            });
        })
        .with_function("recyclable async trait obj", |b| {
            b.iter(|| {
                futures::executor::block_on(async {
                    let mut adder = DynamicRecyclableFutureAsyncTraitAdderImpl::default();
                    for _ in 0..ADDER_ITERATIONS {
                        assert_eq!(25, adder.add_obj(5, 20).await);
                        assert_eq!(25, adder.current());
                    }
                });
            });
        }),
    );
}

fn stream_benches(c: &mut Criterion) {
    c.bench(
        "stream_benches",
        Benchmark::new("no trait", |b| {
            b.iter(|| {
                let mut stream = NoTraitStream::new(STREAM_ITERATIONS);
                futures::executor::block_on(async {
                    while let Some(_item) = stream.next().await {}
                });
            });
        })
        .with_function("async trait obj", |b| {
            b.iter(|| {
                futures::executor::block_on(async {
                    let mut stream = AsyncTraitStreamImpl::new(STREAM_ITERATIONS);
                    while let Some(_item) = stream.next().await {}
                });
            });
        })
        .with_function("recyclable async trait obj", |b| {
            b.iter(|| {
                futures::executor::block_on(async {
                    let mut stream =
                        DynamicRecyclableFutureAsyncTraitStreamImpl::new(STREAM_ITERATIONS);
                    while let Some(_item) = stream.next().await {}
                });
            });
        }),
    );
}

fn nested_stream_benches(c: &mut Criterion) {
    c.bench(
        "nexted_stream_benches",
        Benchmark::new("no trait", |b| {
            b.iter(|| {
                let mut stream = NoTraitWrappingStream::new(STREAM_ITERATIONS);
                futures::executor::block_on(async {
                    while let Some(_item) = stream.next().await {}
                });
            });
        })
        .with_function("async trait obj", |b| {
            b.iter(|| {
                futures::executor::block_on(async {
                    let mut stream = AsyncTraitWrappingStreamImpl::new(STREAM_ITERATIONS);
                    while let Some(_item) = stream.next().await {}
                });
            });
        })
        .with_function("recyclable async trait obj", |b| {
            b.iter(|| {
                futures::executor::block_on(async {
                    let mut stream =
                        DynamicRecyclableFutureAsyncTraitWrappingStreamImpl::new(STREAM_ITERATIONS);
                    while let Some(_item) = stream.next().await {}
                });
            });
        }),
    );
}

criterion_group! {
    name = bench_group;
    config = Criterion::default();
    targets = adder_benches, stream_benches, nested_stream_benches
}
criterion_main!(bench_group);
