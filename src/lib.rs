use std::cmp::Ordering;
use std::ops::{Deref, DerefMut};

pub trait Reducer<T> {
    type Error;

    fn reduce(&self, existing: &mut T, incoming: T) -> Result<(), Self::Error>;
}

impl<T, E, F> Reducer<T> for F where F: Fn(&mut T, T) -> Result<(), E> {
    type Error = E;

    fn reduce(&self, existing: &mut T, incoming: T) -> Result<(), E> {
        (*self)(existing, incoming)
    }
}

enum Switch { StateInit, StateA, StateB, }

pub struct SlicesMergerReduce<T, R> {
    reducer: R,
    switch: Switch,
    union_buf_a: Vec<T>,
    union_buf_b: Vec<T>,
}

pub type SlicesMergerUniq<T> = SlicesMergerReduce<T, fn(&mut T, T) -> Result<(), ()>>;

pub struct SlicesMerger<T>(SlicesMergerUniq<T>);

impl<T> SlicesMerger<T> {
    pub fn new() -> SlicesMerger<T> {
        SlicesMerger(SlicesMergerReduce::new(SlicesMerger::uniq_merge))
    }

    pub fn with_capacity(capacity: usize) -> SlicesMerger<T> {
        SlicesMerger(SlicesMergerReduce::with_capacity(SlicesMerger::uniq_merge, capacity))
    }

    pub fn from(init_vec: Vec<T>) -> SlicesMerger<T> {
        SlicesMerger(SlicesMergerReduce::from(init_vec, SlicesMerger::uniq_merge))
    }

    pub fn from_with_capacity(init_vec: Vec<T>, capacity: usize) -> SlicesMerger<T> {
        SlicesMerger(SlicesMergerReduce::from_with_capacity(init_vec, SlicesMerger::uniq_merge, capacity))
    }

    pub fn add<I>(&mut self, items: I) where T: PartialOrd + PartialEq, I: Iterator<Item = T> {
        match self.0.add(items) {
            Ok(()) => (),
            Err(()) => unreachable!(),
        }
    }

    pub fn finish(self) -> Vec<T> {
        self.0.finish()
    }

    fn uniq_merge(_existing: &mut T, _incoming: T) -> Result<(), ()> {
        Ok(())
    }
}

impl<T> Deref for SlicesMerger<T> {
    type Target = SlicesMergerUniq<T>;

    fn deref(&self) -> &SlicesMergerUniq<T> {
        &self.0
    }
}

impl<T> DerefMut for SlicesMerger<T> {
    fn deref_mut(&mut self) -> &mut SlicesMergerUniq<T> {
        &mut self.0
    }
}

impl<T, R> SlicesMergerReduce<T, R> {
    pub fn new(reducer: R) -> SlicesMergerReduce<T, R> {
        SlicesMergerReduce {
            reducer: reducer,
            switch: Switch::StateInit,
            union_buf_a: Vec::new(),
            union_buf_b: Vec::new(),
        }
    }

    pub fn with_capacity(reducer: R, capacity: usize) -> SlicesMergerReduce<T, R> {
        SlicesMergerReduce {
            reducer: reducer,
            switch: Switch::StateInit,
            union_buf_a: Vec::with_capacity(capacity),
            union_buf_b: Vec::with_capacity(capacity),
        }
    }

    pub fn from(init_vec: Vec<T>, reducer: R) -> SlicesMergerReduce<T, R> {
        SlicesMergerReduce {
            reducer: reducer,
            switch: Switch::StateB,
            union_buf_b: Vec::with_capacity(init_vec.len() + 1),
            union_buf_a: init_vec,
        }
    }

    pub fn from_with_capacity(init_vec: Vec<T>, reducer: R, capacity: usize) -> SlicesMergerReduce<T, R> {
        SlicesMergerReduce {
            reducer: reducer,
            switch: Switch::StateB,
            union_buf_b: Vec::with_capacity(capacity),
            union_buf_a: init_vec,
        }
    }

    pub fn reset(&mut self) {
        self.switch = Switch::StateInit;
        self.union_buf_a.clear();
    }

    pub fn add<I>(&mut self, items: I) -> Result<(), R::Error> where
        T: PartialOrd,
        R: Reducer<T>,
        I: Iterator<Item = T>
    {
        self.add_by(items, |a, b| a.partial_cmp(b))
    }

    pub fn add_by<I, F>(&mut self, items: I, comp: F) -> Result<(), R::Error> where
        R: Reducer<T>,
        I: Iterator<Item = T>,
        F: Fn(&T, &T) -> Option<Ordering>
    {
        match self.switch {
            Switch::StateInit => {
                self.switch = Switch::StateB;
                union_into::<_, ::std::vec::Drain<T>, _, _, _>(&mut self.union_buf_a, None, items, comp, &mut self.reducer)
            },
            Switch::StateA => {
                self.switch = Switch::StateB;
                union_into(&mut self.union_buf_a, Some(self.union_buf_b.drain(..)), items, comp, &mut self.reducer)
            },
            Switch::StateB => {
                self.switch = Switch::StateA;
                union_into(&mut self.union_buf_b, Some(self.union_buf_a.drain(..)), items, comp, &mut self.reducer)
            },
        }
    }

    pub fn finish(self) -> Vec<T> {
        match self.switch {
            Switch::StateA => self.union_buf_b,
            Switch::StateB | Switch::StateInit => self.union_buf_a,
        }
    }
}

impl<T, R> Deref for SlicesMergerReduce<T, R> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        match self.switch {
            Switch::StateA => &self.union_buf_b[..],
            Switch::StateB | Switch::StateInit => &self.union_buf_a[..],
        }
    }
}

fn union_into<T, IS, II, F, R>(target: &mut Vec<T>, source_it: Option<IS>, mut src_b: II, comp: F, reducer: &mut R) -> Result<(), R::Error> where
    IS: Iterator<Item = T>,
    II: Iterator<Item = T>,
    F: Fn(&T, &T) -> Option<Ordering>,
    R: Reducer<T>
{
    target.clear();
    match source_it {
        None =>
            target.extend(src_b),
        Some(mut src_a) => {
            let mut surface = (src_a.next(), src_b.next());
            loop {
                surface =
                    match surface {
                        (None, None) =>
                            break,
                        (None, Some(entry_b)) => {
                            target.push(entry_b);
                            (None, src_b.next())
                        },
                        (Some(entry_a), None) => {
                            target.push(entry_a);
                            (src_a.next(), None)
                        },
                        (Some(mut entry_a), Some(entry_b)) => match comp(&entry_a, &entry_b) {
                            Some(Ordering::Equal) => {
                                try!(reducer.reduce(&mut entry_a, entry_b));
                                target.push(entry_a);
                                (src_a.next(), src_b.next())
                            },
                            Some(Ordering::Less) | None => {
                                target.push(entry_a);
                                (src_a.next(), Some(entry_b))
                            }
                            Some(Ordering::Greater) => {
                                target.push(entry_b);
                                (Some(entry_a), src_b.next())
                            },
                        },
                    };
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::{SlicesMerger, SlicesMergerReduce};

    #[test]
    fn basic() {
        let mut merger = SlicesMerger::new();
        assert_eq!(merger.len(), 0);
        merger.add([1, 3, 4, 5, 7].iter().cloned());
        assert_eq!(&merger[..], &[1, 3, 4, 5, 7]);
        merger.add([6].iter().cloned());
        assert_eq!(&merger[..], &[1, 3, 4, 5, 6, 7]);
        merger.add([0].iter().cloned());
        assert_eq!(&merger[..], &[0, 1, 3, 4, 5, 6, 7]);
        merger.add([8].iter().cloned());
        assert_eq!(&merger[..], &[0, 1, 3, 4, 5, 6, 7, 8]);
        merger.add([2, 4, 5, 6, 7, 8, 9].iter().cloned());
        assert_eq!(&merger[..], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        merger.reset();
        merger.add([4].iter().cloned());
        assert_eq!(&merger[..], &[4]);
    }

    #[test]
    fn reduce() {
        let mut merger =
            SlicesMergerReduce::new(|&mut (_, ref mut total): &mut (_, _), (_, inc)| { *total += inc; Ok::<(), ()>(()) });
        assert_eq!(merger.len(), 0);
        merger.add_by([1, 3, 4, 5, 7].iter().cloned().map(|v| (v, 1)), |&(ref a, _), &(ref b, _)| a.partial_cmp(b)).unwrap();
        assert_eq!(&merger[..], &[(1, 1), (3, 1), (4, 1), (5, 1), (7, 1)]);
        merger.add_by([2, 4, 5, 6, 7, 8, 9].iter().cloned().map(|v| (v, 1)), |&(ref a, _), &(ref b, _)| a.partial_cmp(b)).unwrap();
        assert_eq!(&merger[..], &[(1, 1), (2, 1), (3, 1), (4, 2), (5, 2), (6, 1), (7, 2), (8, 1), (9, 1)]);
    }

    #[test]
    fn from_finish() {
        let mut merger = SlicesMerger::from(vec![1, 3, 4, 5, 7]);
        merger.add([2, 4, 5, 6, 7, 8, 9].iter().cloned());
        assert_eq!(merger.finish(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}
