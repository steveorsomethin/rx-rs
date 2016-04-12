# rx-rs

Quick thoughts:
* I've got completion totally wrong right now since this implementation is about 2 years old.
* The Iterable/Observable containers might be a non-starter in rust due to ownership issues. See these comments I made in /r/rust for more context: https://www.reddit.com/r/rust/comments/2s08aa/cloning_unboxed_closures_which_own_their/ https://www.reddit.com/r/rust/comments/2sn50q/copyable_closures/cnrcjtj?context=3
  * Functions can implement Observable, which might be a way out.
* Most operations shouldn't need to allocate, and Observer chains should be able to carry similar overhead to Iterator chains in the std lib. Zip would allocate due to buffering. Merge might not need to if I make its internal disposable list an enum of say, One|Many where One is inside the memory space of the Merge struct, and we lazily allocate a vector on the heap for larger numbers of merged observables. You might also be able to arbitrarily size the number of disposables to track statically once numeric generic args land. There's an RFC for that but I can't find it.
* [Rotor](https://github.com/tailhook/rotor) might be a good place to start for a scheduler.
