use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// A boolean flag that can be shared between threads
#[derive(Clone)]
pub struct Flag {
    flag: Arc<AtomicBool>,
}

impl Flag {
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// set the flag
    pub fn set(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// reset the flag
    pub fn reset(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }

    /// check if the flag is set
    pub fn is_set(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

pub fn reinterleave<T:Copy>(i:&[T], o:&mut [T], nch:usize, should_interleave:bool) {
    let nsm = i.len() / nch; 
    for ch in 0..nch {
        for sm in 0..nsm {
            let deinterleaved = sm + ch * nsm;
            let interleaved = sm * nch + ch;
            let (idx, odx) = if should_interleave { (interleaved, deinterleaved) } else { (deinterleaved, interleaved) };
            o[odx] = i[idx];
        }
    }
}

pub fn interleave<T:Copy>(i:&[T], o:&mut[T], nch:usize) {
    reinterleave(i, o, nch, true);
}

pub fn deinterleave<T:Copy>(i:&[T], o:&mut[T], nch:usize) {
    reinterleave(i, o, nch, false);
}

