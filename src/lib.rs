use std::ops::Deref;
use std::cmp::Ordering;

enum Switch { StateInit, StateA, StateB, }

pub struct SlicesMerger<T> {
    switch: Switch,
    union_buf_a: Vec<T>,
    union_buf_b: Vec<T>,
}

impl<T> SlicesMerger<T> {
    pub fn new() -> SlicesMerger<T> {
        SlicesMerger {
            switch: Switch::StateInit,
            union_buf_a: Vec::new(),
            union_buf_b: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> SlicesMerger<T> {
        SlicesMerger {
            switch: Switch::StateInit,
            union_buf_a: Vec::with_capacity(capacity),
            union_buf_b: Vec::with_capacity(capacity),
        }
    }

    pub fn from(init_vec: Vec<T>) -> SlicesMerger<T> {
        SlicesMerger {
            switch: Switch::StateB,
            union_buf_b: Vec::with_capacity(init_vec.len() + 1),
            union_buf_a: init_vec,
        }
    }

    pub fn from_with_capacity(init_vec: Vec<T>, capacity: usize) -> SlicesMerger<T> {
        SlicesMerger {
            switch: Switch::StateB,
            union_buf_b: Vec::with_capacity(capacity),
            union_buf_a: init_vec,
        }
    }

    pub fn reset(&mut self) {
        self.switch = Switch::StateInit;
        self.union_buf_a.clear();
    }

    pub fn add<I>(&mut self, items: I) where T: PartialOrd, I: Iterator<Item = T>
    {
        self.add_by(items, |a, b| a.partial_cmp(b), |_, _| ())
    }

    pub fn add_by<I, FC, FR>(&mut self, items: I, comp: FC, reduce: FR) where
        I: Iterator<Item = T>,
        FC: Fn(&T, &T) -> Option<Ordering>,
        FR: FnMut(&mut T, T)
    {
        match self.switch {
            Switch::StateInit => {
                self.switch = Switch::StateB;
                union_into::<_, ::std::vec::Drain<T>, _, _, _>(&mut self.union_buf_a, None, items, comp, reduce)
            },
            Switch::StateA => {
                self.switch = Switch::StateB;
                union_into(&mut self.union_buf_a, Some(self.union_buf_b.drain(..)), items, comp, reduce)
            },
            Switch::StateB => {
                self.switch = Switch::StateA;
                union_into(&mut self.union_buf_b, Some(self.union_buf_a.drain(..)), items, comp, reduce)
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

impl<T> Deref for SlicesMerger<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        match self.switch {
            Switch::StateA => &self.union_buf_b[..],
            Switch::StateB | Switch::StateInit => &self.union_buf_a[..],
        }
    }
}

fn union_into<T, IS, II, FC, FR>(target: &mut Vec<T>, source_it: Option<IS>, mut src_b: II, comp: FC, mut reduce: FR) where
    IS: Iterator<Item = T>,
    II: Iterator<Item = T>,
    FC: Fn(&T, &T) -> Option<Ordering>,
    FR: FnMut(&mut T, T)
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
                            return,
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
                                reduce(&mut entry_a, entry_b);
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
}

#[cfg(test)]
mod test {
    use super::SlicesMerger;

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
        fn comp(&(ref a, _): &(isize, isize), &(ref b, _): &(isize, isize)) -> Option<::std::cmp::Ordering> { a.partial_cmp(b) }
        fn reduce(&mut (_, ref mut total): &mut (isize, isize), (_, inc): (isize, isize)) { *total += inc }

        let mut merger = SlicesMerger::new();
        assert_eq!(merger.len(), 0);
        merger.add_by([1, 3, 4, 5, 7].iter().cloned().map(|v| (v, 1)), comp, reduce);
        assert_eq!(&merger[..], &[(1, 1), (3, 1), (4, 1), (5, 1), (7, 1)]);
        merger.add_by([2, 4, 5, 6, 7, 8, 9].iter().cloned().map(|v| (v, 1)), comp, reduce);
        assert_eq!(&merger[..], &[(1, 1), (2, 1), (3, 1), (4, 2), (5, 2), (6, 1), (7, 2), (8, 1), (9, 1)]);
    }

    #[test]
    fn from_finish() {
        let mut merger = SlicesMerger::from(vec![1, 3, 4, 5, 7]);
        merger.add([2, 4, 5, 6, 7, 8, 9].iter().cloned());
        assert_eq!(merger.finish(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}
