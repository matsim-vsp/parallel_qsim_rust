use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub trait EndTime {
    fn end_time(&self, now: u32) -> u32;
}

struct Entry<T>
where
    T: EndTime,
{
    end_time: u32,
    value: T,
}

impl<T> PartialEq<Self> for Entry<T>
where
    T: EndTime,
{
    fn eq(&self, _other: &Self) -> bool {
        false // how bad is this...
    }
}

impl<T> Eq for Entry<T> where T: EndTime {}

impl<T> PartialOrd<Self> for Entry<T>
where
    T: EndTime,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Entry<T>
where
    T: EndTime,
{
    fn cmp(&self, other: &Self) -> Ordering {
        other.end_time.cmp(&self.end_time)
    }
}

pub struct TimeQueue<T>
where
    T: EndTime,
{
    q: BinaryHeap<Entry<T>>,
}

impl<T> TimeQueue<T>
where
    T: EndTime,
{
    pub fn new() -> Self {
        TimeQueue {
            q: BinaryHeap::new(),
        }
    }

    pub fn add(&mut self, value: T, now: u32) {
        let end_time = value.end_time(now);
        self.q.push(Entry { end_time, value });
    }

    pub fn pop(&mut self, now: u32) -> Vec<T> {
        let mut result: Vec<T> = Vec::new();

        while let Some(entry_ref) = self.q.peek() {
            if entry_ref.end_time <= now {
                let entry = self.q.pop().unwrap();
                result.push(entry.value);
            } else {
                break;
            }
        }

        result
    }
}
