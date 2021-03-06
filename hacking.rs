#![feature(zero_one)]
#![feature(step_trait)]
use std::num::{One};
use std::sync::{Arc, Mutex};
use std::marker::PhantomData;
use std::marker::Sized;
use std::iter::Step;
use std::ops::{Add};

enum IterationResult {
    Stop,
    Continue
}

use IterationResult::{Stop, Continue};

trait Observer {
    type Item;

    fn next(&mut self, val: Self::Item) -> IterationResult;
    fn completed(&mut self);
}

trait Observable {
    type Item;

    fn subscribe<N>(&self, observer: N) where N: Observer<Item=Self::Item> + Send + Sync;

    #[inline]
    fn map<B, F>(self, f: F) -> MapObservable<F, Self>
        where F: Fn(Self::Item) -> B,
              Self: Sized {
        MapObservable {f: Arc::new(f), source: self}
    }

    #[inline]
    fn take(self, count: usize) -> TakeObservable<Self>
        where Self: Sized {
        TakeObservable {count: count, source: self}
    }

    #[inline]
    fn merge_all<U>(self) -> MergeAllObservable<Self, U>
        where Self: Sized {
        MergeAllObservable {source: self, _marker: PhantomData}
    }

    #[inline]
    fn flat_map<U, F>(self, f: F) -> MergeAllObservable<MapObservable<F, Self>, U>
        where F: Fn(Self::Item) -> U,
              U: Observable<Item=Self::Item> + Send + Sync,
              Self: Sized {
        MergeAllObservable {source: self.map(f), _marker: PhantomData}
    }
}

struct RangeObservable<A> {
    start: A,
    end: A
}

impl<A> Observable for RangeObservable<A> 
    where A: Step + One + Clone,
    for<'a> &'a A: Add<&'a A, Output = A> {
    type Item = A;

    #[inline]
    fn subscribe<N>(&self, mut observer: N)
        where N: Observer<Item=Self::Item> + Send + Sync {
        let mut state = self.start.clone();

        loop {
            let result = observer.next(state.clone());
            state = match result {
                Stop => self.end.clone(),
                Continue => &state + &A::one()
            };

            if state >= self.end {
                break;
            }
        }

        observer.completed();
    }
}

fn range<A: One>(start: A, end: A) -> RangeObservable<A> {
    RangeObservable {start: start, end: end}
}

struct ValueObservable<A> {
    value: A
}

impl<A> Observable for ValueObservable<A>
    where A: Clone {
    type Item = A;

    #[inline]
    fn subscribe<N>(&self, mut observer: N)
        where N: Observer<Item=Self::Item> + Send + Sync {

        observer.next(self.value.clone());
        observer.completed();
    }
}

fn value<A: Clone>(value: A) -> ValueObservable<A> {
    ValueObservable {value: value}
}

//////////////Map
struct MapObservable<F, S> {
    f: Arc<F>,
    source: S
}

impl<B, F, S> Observable for MapObservable<F, S>
    where S::Item: Send + Sync,
          F: Fn(S::Item) -> B + Send + Sync,
          S: Observable {
    type Item = B;

    fn subscribe<N>(&self, observer: N)
        where N: Observer<Item=Self::Item> + Send + Sync {
        self.source.subscribe(MapObserver {f: self.f.clone(), observer: observer, _marker: PhantomData});
    }
}

struct MapObserver<F, N, B> {
    f: Arc<F>,
    observer: N,
    _marker: PhantomData<B>
}

impl<B, N, F> Observer for MapObserver<F, N, B>
    where N: Observer,
          F: Fn(B) -> N::Item {
    type Item = B;

    fn next(&mut self, val: Self::Item) -> IterationResult {
        self.observer.next((self.f)(val))
    }

    fn completed(&mut self) {
        self.observer.completed();
    }
}
//Map//////////////

//////////////MergeAll
struct SharedObserver<A, N> {
    observer: Arc<Mutex<N>>,
    _marker: PhantomData<A>
}

impl<A, N> Observer for SharedObserver<A, N>
    where A: Send + Sync,
          N: Observer<Item=A> + Send + Sync {
    type Item = A;

    fn next(&mut self, val: Self::Item) -> IterationResult {
        self.observer.lock().unwrap().next(val)
    }

    fn completed(&mut self) {
        self.observer.lock().unwrap().completed();
    }
}

struct MergeAllObservable<S, U> {
    source: S,
    _marker: PhantomData<U>
}

impl<U, S> Observable for MergeAllObservable<S, U>
    where S::Item: Observable + Send + Sync,
          S: Observable<Item=U> + Send + Sync,
          U: Observable + Send + Sync,
          U::Item: Send + Sync {
    type Item = U::Item;

    fn subscribe<N>(&self, observer: N)
        where N: Observer<Item=Self::Item> + Send + Sync {
        self.source.subscribe(MergeAllObserver {observer: Arc::new(Mutex::new(observer)), _marker: PhantomData});
    }
}

struct MergeAllObserver<N, U> {
    observer: Arc<Mutex<N>>,
    _marker: PhantomData<U>
}

impl<N, U> Observer for MergeAllObserver<N, U>
    where U: Observable + Send + Sync,
          U::Item: Send + Sync,
          N: Observer<Item=U::Item> + Send + Sync {
    type Item = U;

    fn next(&mut self, val: U) -> IterationResult {
        val.subscribe(SharedObserver {
            observer: self.observer.clone(),
            _marker: PhantomData
        });
        Continue
    }

    fn completed(&mut self) {
        self.observer.lock().unwrap().completed();
    }
}
//MergeAll//////////////

//////////////Take
struct TakeObservable<S> {
    count: usize,
    source: S
}

impl<S> Observable for TakeObservable<S>
    where S: Observable,
          S::Item: Send + Sync {
    type Item = S::Item;

    #[inline]
    fn subscribe<N>(&self, observer: N)
        where N: Observer<Item=Self::Item> + Send + Sync {
        self.source.subscribe(TakeObserver {remaining: self.count.clone(), observer: observer, _marker: PhantomData});
    }
}

struct TakeObserver<A, N> {
    remaining: usize,
    observer: N,
    _marker: PhantomData<A>
}

impl<A, N> Observer for TakeObserver<A, N>
    where N: Observer<Item=A> {
    type Item = A;

    #[inline]
    fn next(&mut self, val: Self::Item) -> IterationResult {
        let result = if self.remaining > 0 {
            self.observer.next(val)
        } else {
            Stop
        };

        self.remaining = match result {
            Stop => 0,
            Continue => self.remaining - 1
        };

        if self.remaining > 0 {
            Continue
        } else {
            Stop
        }
    }

    fn completed(&mut self) {
        self.observer.completed();
    }
}
//Take//////////////

struct AnonymousObserver<F, B> {
    next: F,
    _marker: PhantomData<B>
}

impl<F, B> Observer for AnonymousObserver<F, B>
    where F: Fn(B) {
    type Item = B;

    fn next(&mut self, val: Self::Item) -> IterationResult {
        (self.next)(val);
        Continue
    }

    fn completed(&mut self) {
        println!("Called completed");
    }
}

fn main() {
    range(0, 10).
        flat_map(|a| value(a * 10)).
        take(3).
        map(|a| a + 5).
        subscribe(AnonymousObserver {
            next: |a| println!("Got {}", a),
            _marker: PhantomData
        });
}