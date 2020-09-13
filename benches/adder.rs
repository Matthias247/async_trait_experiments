use async_trait::async_trait;
use async_trait_experiments::{DynamicFuture, RecyclableFutureAllocator};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

const NR_YIELDS: usize = 0;

#[derive(Default)]
pub struct NoTraitAdder {
    pub current: u32,
}

impl NoTraitAdder {
    pub async fn add_obj(&mut self, a: u32, b: u32) -> u32 {
        let mut storage = [0u32; 64];
        let result = a + b;
        Yielder::new(NR_YIELDS).await;
        self.current = result;
        storage[4] = result;
        storage[4]
    }
}

#[async_trait]
pub trait AsyncTraitAdder {
    async fn add_obj(&mut self, a: u32, b: u32) -> u32;
}

#[derive(Default)]
pub struct AsyncTraitAdderImpl {
    pub current: u32,
}

#[async_trait]
impl AsyncTraitAdder for AsyncTraitAdderImpl {
    async fn add_obj(&mut self, a: u32, b: u32) -> u32 {
        let mut storage = [0u32; 64];
        let result = a + b;
        Yielder::new(NR_YIELDS).await;
        self.current = result;
        storage[4] = result;
        storage[4]
    }
}

pub trait BoxPinFutureTraitAdder {
    fn add_obj<'a>(&'a mut self, a: u32, b: u32) -> Pin<Box<dyn Future<Output = u32> + 'a>>;
}

#[derive(Default)]
pub struct BoxPinFutureTraitAdderImpl {
    pub current: u32,
}

impl BoxPinFutureTraitAdder for BoxPinFutureTraitAdderImpl {
    fn add_obj<'a>(&'a mut self, a: u32, b: u32) -> Pin<Box<dyn Future<Output = u32> + 'a>> {
        Box::pin(async move {
            let mut storage = [0u32; 64];
            let result = a + b;
            Yielder::new(NR_YIELDS).await;
            self.current = result;
            storage[4] = result;
            storage[4]
        })
    }
}

pub trait DynamicFutureAsyncTraitAdder {
    fn add_obj<'a>(&'a mut self, a: u32, b: u32) -> DynamicFuture<'a, u32>;
}

#[derive(Default)]
struct AdderState {
    current: u32,
}

#[derive(Default)]
pub struct DynamicRecyclableFutureAsyncTraitAdderImpl {
    state: AdderState,
    add_obj_recycler: RecyclableFutureAllocator,
}

impl DynamicRecyclableFutureAsyncTraitAdderImpl {
    pub fn current(&self) -> u32 {
        self.state.current
    }
}

impl DynamicFutureAsyncTraitAdder for DynamicRecyclableFutureAsyncTraitAdderImpl {
    fn add_obj<'a>(&'a mut self, a: u32, b: u32) -> DynamicFuture<'a, u32> {
        let state = &mut self.state;

        self.add_obj_recycler.allocate(async move {
            let mut storage = [0u32; 64];
            let result = a + b;
            Yielder::new(NR_YIELDS).await;
            state.current = result;
            storage[4] = result;
            storage[4]
        })
    }
}

/// A Future which yields to the executor for a given amount of iterations
/// and resolves after this
pub struct Yielder {
    iter: usize,
}

impl Yielder {
    pub fn new(iter: usize) -> Yielder {
        Yielder { iter }
    }
}

impl Future for Yielder {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.iter == 0 {
            Poll::Ready(())
        } else {
            self.iter -= 1;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
