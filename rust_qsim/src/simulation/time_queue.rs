use crate::simulation::id::serializable_type::StableTypeId;
use crate::simulation::id::Id;
use nohash_hasher::IntMap;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub trait EndTime {
    fn end_time(&self, now: u32) -> u32;
}

pub trait Identifiable<I: StableTypeId> {
    fn id(&self) -> &Id<I>;
}

struct Entry<T>
where
    T: EndTime,
{
    end_time: u32,
    order: usize,
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
        // First compare by end_time (reverse for min-heap)
        // Then use order as secondary sort key (also reverse for FIFO within same time)
        other.end_time.cmp(&self.end_time)
            .then_with(|| other.order.cmp(&self.order))
    }
}

pub struct TimeQueue<T, I>
where
    T: EndTime,
{
    q: BinaryHeap<Entry<T>>,
    counter: usize,
    _phantom: std::marker::PhantomData<I>,
}

impl<T, I> Default for TimeQueue<T, I>
where
    T: EndTime,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, I> TimeQueue<T, I>
where
    T: EndTime,
{
    pub fn new() -> Self {
        TimeQueue {
            q: BinaryHeap::new(),
            counter: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn add(&mut self, value: T, now: u32) {
        let end_time = value.end_time(now);
        let order = self.counter;
        self.counter += 1;
        self.q.push(Entry { end_time, order, value });
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

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.q.len()
    }
}

struct ValueWrap<I: StableTypeId> {
    id: Id<I>,
    end_time: u32,
}

impl<I: StableTypeId> ValueWrap<I> {
    fn new(id: Id<I>, end_time: u32) -> Self {
        ValueWrap { id, end_time }
    }
}

impl<I: StableTypeId> EndTime for ValueWrap<I> {
    fn end_time(&self, _: u32) -> u32 {
        self.end_time
    }
}

/// This is a mutable version of TimeQueue. It allows to mutate the values in the queue.
/// It is a logical error to mutate the end_time of the value such that the order of the queue is changed.
/// TODO taxi driver needs to be able to change his end_time such that order is changed
pub struct MutTimeQueue<T, I>
where
    T: EndTime + Identifiable<I>,
    I: StableTypeId,
{
    q: TimeQueue<ValueWrap<I>, I>,
    cache: IntMap<Id<I>, T>,
}

impl<T, I> Default for MutTimeQueue<T, I>
where
    T: EndTime + Identifiable<I>,
    I: StableTypeId + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, I> MutTimeQueue<T, I>
where
    T: EndTime + Identifiable<I>,
    I: StableTypeId + 'static,
{
    pub fn new() -> Self {
        MutTimeQueue {
            q: TimeQueue::new(),
            cache: IntMap::default(),
        }
    }

    pub fn add(&mut self, value: T, now: u32) {
        let id = value.id();
        self.q
            .add(ValueWrap::new(id.clone(), value.end_time(now)), now);
        self.cache.insert(id.clone(), value);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestItem {
        id: u32,
        end: u32,
    }

    impl EndTime for TestItem {
        fn end_time(&self, _now: u32) -> u32 {
            self.end
        }
    }

    #[test]
    fn test_time_queue_stable_ordering() {
        let mut queue: TimeQueue<TestItem, ()> = TimeQueue::new();
        
        // Add multiple items with the same end time
        // They should be popped in the order they were added (FIFO)
        queue.add(TestItem { id: 1, end: 10 }, 0);
        queue.add(TestItem { id: 2, end: 10 }, 0);
        queue.add(TestItem { id: 3, end: 10 }, 0);
        
        let results = queue.pop(10);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, 1);
        assert_eq!(results[1].id, 2);
        assert_eq!(results[2].id, 3);
    }

    #[test]
    fn test_time_queue_time_ordering_priority() {
        let mut queue: TimeQueue<TestItem, ()> = TimeQueue::new();
        
        // Add items with different end times
        // They should be popped in time order first
        queue.add(TestItem { id: 1, end: 15 }, 0);
        queue.add(TestItem { id: 2, end: 10 }, 0);
        queue.add(TestItem { id: 3, end: 20 }, 0);
        queue.add(TestItem { id: 4, end: 10 }, 0); // Same as id:2
        
        let results = queue.pop(10);
        assert_eq!(results.len(), 2);
        // Time 10 items should come out in order they were added
        assert_eq!(results[0].id, 2);
        assert_eq!(results[1].id, 4);
        
        let results = queue.pop(15);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);
        
        let results = queue.pop(20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 3);
    }
}
