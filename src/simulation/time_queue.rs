use nohash_hasher::IntMap;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub trait EndTime {
    fn end_time(&self, now: u32) -> u32;
}

pub trait Identifiable {
    fn id(&self) -> u64;
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

#[derive(Default)]
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

    pub fn len(&self) -> usize {
        self.q.len()
    }
}

#[derive(Default)]
struct ValueWrap {
    id: u64,
    end_time: u32,
}

impl ValueWrap {
    fn new(id: u64, end_time: u32) -> Self {
        ValueWrap { id, end_time }
    }
}

impl EndTime for ValueWrap {
    fn end_time(&self, _: u32) -> u32 {
        self.end_time
    }
}

/// This is a mutable version of TimeQueue. It allows to mutate the values in the queue.
/// It is a logical error to mutate the end_time of the value such that the order of the queue is changed.
/// TODO taxi driver needs to be able to change his end_time such that order is changed
#[derive(Default)]
pub struct MutTimeQueue<T>
where
    T: EndTime + Identifiable,
{
    q: TimeQueue<ValueWrap>,
    cache: IntMap<u64, T>,
}

impl<T> MutTimeQueue<T>
where
    T: EndTime + Identifiable,
{
    pub fn new() -> Self {
        MutTimeQueue {
            q: TimeQueue::new(),
            cache: IntMap::default(),
        }
    }

    pub fn add(&mut self, value: T, now: u32) {
        let id = value.id();
        self.q.add(ValueWrap::new(id, value.end_time(now)), now);
        self.cache.insert(id, value);
    }

    pub fn pop(&mut self, now: u32) -> Vec<T> {
        let ids = self.q.pop(now);
        let mut result: Vec<T> = Vec::new();

        for id in ids {
            let value = self.cache.remove(&id.id).unwrap();
            result.push(value);
        }

        result
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.cache.values_mut()
    }
}
