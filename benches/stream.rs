use async_trait::async_trait;

pub use async_trait_experiments::{box_future, DynamicFuture, RecyclableFutureAllocator};

pub struct NoTraitStream {
    pub current: u32,
}

impl NoTraitStream {
    pub fn new(current: u32) -> Self {
        Self { current }
    }

    pub async fn next(&mut self) -> Option<u32> {
        if self.current == 0 {
            None
        } else {
            self.current -= 1;
            Some(self.current)
        }
    }
}

pub struct NoTraitWrappingStream {
    pub inner: NoTraitStream,
}

impl NoTraitWrappingStream {
    pub fn new(current: u32) -> Self {
        Self {
            inner: NoTraitStream::new(current),
        }
    }

    pub async fn next(&mut self) -> Option<u32> {
        self.inner.next().await
    }
}

#[async_trait(?Send)]
pub trait AsyncTraitStream {
    async fn next(&mut self) -> Option<u32>;
}

#[derive(Default)]
pub struct AsyncTraitStreamImpl {
    pub current: u32,
}

impl AsyncTraitStreamImpl {
    pub fn new(current: u32) -> Self {
        Self { current }
    }
}

#[async_trait(?Send)]
impl AsyncTraitStream for AsyncTraitStreamImpl {
    async fn next(&mut self) -> Option<u32> {
        if self.current == 0 {
            None
        } else {
            self.current -= 1;
            Some(self.current)
        }
    }
}

pub struct AsyncTraitWrappingStreamImpl {
    inner: Box<dyn AsyncTraitStream>,
}

impl AsyncTraitWrappingStreamImpl {
    pub fn new(current: u32) -> Self {
        Self {
            inner: Box::new(AsyncTraitStreamImpl::new(current)),
        }
    }
}

#[async_trait(?Send)]
impl AsyncTraitStream for AsyncTraitWrappingStreamImpl {
    async fn next(&mut self) -> Option<u32> {
        self.inner.next().await
    }
}

pub trait DynamicFutureAsyncTraitStream {
    fn next<'a>(&'a mut self) -> DynamicFuture<'a, Option<u32>>;
}

#[derive(Default)]
struct StreamState {
    current: u32,
}

#[derive(Default)]
pub struct DynamicRecyclableFutureAsyncTraitStreamImpl {
    state: StreamState,
    next_recycler: RecyclableFutureAllocator,
}

impl DynamicRecyclableFutureAsyncTraitStreamImpl {
    pub fn new(current: u32) -> Self {
        Self {
            state: StreamState { current },
            next_recycler: Default::default(),
        }
    }
}

impl DynamicFutureAsyncTraitStream for DynamicRecyclableFutureAsyncTraitStreamImpl {
    fn next<'a>(&'a mut self) -> DynamicFuture<'a, Option<u32>> {
        let state = &mut self.state;

        self.next_recycler.allocate(async move {
            if state.current == 0 {
                None
            } else {
                state.current -= 1;
                Some(state.current)
            }
        })
    }
}

#[derive(Default)]
pub struct DynamicBoxedFutureAsyncTraitStreamImpl {
    state: StreamState,
}

impl DynamicBoxedFutureAsyncTraitStreamImpl {
    pub fn new(current: u32) -> Self {
        Self {
            state: StreamState { current },
        }
    }
}

impl DynamicFutureAsyncTraitStream for DynamicBoxedFutureAsyncTraitStreamImpl {
    fn next<'a>(&'a mut self) -> DynamicFuture<'a, Option<u32>> {
        let state = &mut self.state;

        box_future(async move {
            if state.current == 0 {
                None
            } else {
                state.current -= 1;
                Some(state.current)
            }
        })
    }
}

struct WrappingStreamState {
    inner: Box<dyn DynamicFutureAsyncTraitStream>,
}

pub struct DynamicRecyclableFutureAsyncTraitWrappingStreamImpl {
    state: WrappingStreamState,
    next_recycler: RecyclableFutureAllocator,
}

impl DynamicRecyclableFutureAsyncTraitWrappingStreamImpl {
    pub fn new(current: u32) -> Self {
        Self {
            state: WrappingStreamState {
                inner: Box::new(DynamicRecyclableFutureAsyncTraitStreamImpl::new(current)),
            },
            next_recycler: Default::default(),
        }
    }
}

impl DynamicFutureAsyncTraitStream for DynamicRecyclableFutureAsyncTraitWrappingStreamImpl {
    fn next<'a>(&'a mut self) -> DynamicFuture<'a, Option<u32>> {
        let state = &mut self.state;

        self.next_recycler
            .allocate(async move { state.inner.next().await })
    }
}

pub struct DynamicBoxedFutureAsyncTraitWrappingStreamImpl {
    state: WrappingStreamState,
}

impl DynamicBoxedFutureAsyncTraitWrappingStreamImpl {
    pub fn new(current: u32) -> Self {
        Self {
            state: WrappingStreamState {
                inner: Box::new(DynamicRecyclableFutureAsyncTraitStreamImpl::new(current)),
            },
        }
    }
}

impl DynamicFutureAsyncTraitStream for DynamicBoxedFutureAsyncTraitWrappingStreamImpl {
    fn next<'a>(&'a mut self) -> DynamicFuture<'a, Option<u32>> {
        let state = &mut self.state;

        box_future(async move { state.inner.next().await })
    }
}
