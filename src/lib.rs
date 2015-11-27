use std::ops::Deref;

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

    pub fn reset(&mut self) {
        self.switch = Switch::StateInit;
    }

    pub fn add(&mut self, items: &[T]) where T: Clone + PartialOrd + PartialEq {
        match self.switch {
            Switch::StateInit => { self.switch = Switch::StateB; union_into(&mut self.union_buf_a, None, items) },
            Switch::StateA => { self.switch = Switch::StateB; union_into(&mut self.union_buf_a, Some(&self.union_buf_b[..]), items) },
            Switch::StateB => { self.switch = Switch::StateA; union_into(&mut self.union_buf_b, Some(&self.union_buf_a[..]), items) },
        };
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

fn union_into<T>(target_buf: &mut Vec<T>, source_buf: Option<&[T]>, items: &[T]) where T: Clone + PartialOrd + PartialEq {
    target_buf.clear();
    match source_buf {
        None =>
            target_buf.extend(items.iter().cloned()),
        Some(existing_items) if existing_items.is_empty() =>
            target_buf.extend(items.iter().cloned()),
        Some(existing_items) if items.is_empty() =>
            target_buf.extend(existing_items.iter().cloned()),
        Some(existing_items) if items[0] > existing_items[existing_items.len() - 1] => {
            target_buf.extend(existing_items.iter().cloned());
            target_buf.extend(items.iter().cloned());
        },
        Some(existing_items) if existing_items[0] > items[items.len() - 1] => {
            target_buf.extend(items.iter().cloned());
            target_buf.extend(existing_items.iter().cloned());
        },
        Some(existing_items) => {
            let mut index_a = 0;
            let mut index_b = 0;

            while (index_a < existing_items.len()) || (index_b < items.len()) {
                if index_a >= existing_items.len() {
                    target_buf.push(items[index_b].clone());
                    index_b = index_b + 1;
                } else if index_b >= items.len() {
                    target_buf.push(existing_items[index_a].clone());
                    index_a = index_a + 1;
                } else if existing_items[index_a] == items[index_b] {
                    target_buf.push(existing_items[index_a].clone());
                    index_a = index_a + 1;
                    index_b = index_b + 1;
                } else if existing_items[index_a] < items[index_b] {
                    target_buf.push(existing_items[index_a].clone());
                    index_a = index_a + 1;
                } else {
                    target_buf.push(items[index_b].clone());
                    index_b = index_b + 1;
                }
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
        merger.add(&[1, 3, 4, 5, 7]);
        assert_eq!(&merger[..], &[1, 3, 4, 5, 7]);
        merger.add(&[6]);
        assert_eq!(&merger[..], &[1, 3, 4, 5, 6, 7]);
        merger.add(&[0]);
        assert_eq!(&merger[..], &[0, 1, 3, 4, 5, 6, 7]);
        merger.add(&[8]);
        assert_eq!(&merger[..], &[0, 1, 3, 4, 5, 6, 7, 8]);
        merger.add(&[2, 4, 5, 6, 7, 8, 9]);
        assert_eq!(&merger[..], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        merger.reset();
        merger.add(&[4]);
        assert_eq!(&merger[..], &[4]);
    }
}

