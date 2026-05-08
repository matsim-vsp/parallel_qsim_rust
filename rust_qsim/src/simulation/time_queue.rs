use crate::simulation::id::Id;
use crate::simulation::id::serializable_type::StableTypeId;
use crate::simulation::time::Tick;
use nohash_hasher::IntMap;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub(crate) trait EndTick {
    fn end_tick(&self, now: Tick) -> Tick;
}

pub trait Identifiable<I: StableTypeId> {
    fn id(&self) -> &Id<I>;
}

struct Entry<T>
where
    T: EndTick,
{
    end_time: Tick,
    order: usize,
    value: T,
}

impl<T> PartialEq<Self> for Entry<T>
where
    T: EndTick,
{
    fn eq(&self, other: &Self) -> bool {
        self.end_time == other.end_time && self.order == other.order
    }
}

impl<T> Eq for Entry<T> where T: EndTick {}

impl<T> PartialOrd<Self> for Entry<T>
where
    T: EndTick,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Entry<T>
where
    T: EndTick,
{
    fn cmp(&self, other: &Self) -> Ordering {
        // First compare by end_time (reverse for min-heap)
        // Then use order as secondary sort key (also reverse for FIFO within same time)
        other
            .end_time
            .cmp(&self.end_time)
            .then_with(|| other.order.cmp(&self.order))
    }
}

/// TimeQueue provides a priority queue ordered by time with stable FIFO ordering
/// for entries with the same time.
///
/// Note: The internal counter will wrap around after usize::MAX insertions (2^64 on 64-bit systems).
/// This is acceptable for simulation purposes as it would take an astronomically large number
/// of insertions to overflow, and wrapping would only affect ordering in the unlikely event
/// of having entries with both the same time and counter values after overflow.
pub(crate) struct TimeQueue<T, I>
where
    T: EndTick,
{
    q: BinaryHeap<Entry<T>>,
    counter: usize,
    _phantom: std::marker::PhantomData<I>,
}

impl<T, I> Default for TimeQueue<T, I>
where
    T: EndTick,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, I> TimeQueue<T, I>
where
    T: EndTick,
{
    pub(crate) fn new() -> Self {
        TimeQueue {
            q: BinaryHeap::new(),
            counter: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    pub(crate) fn add(&mut self, value: T, now: Tick) {
        let end_time = value.end_tick(now);
        let order = self.counter;
        self.counter = self.counter.wrapping_add(1);
        self.q.push(Entry {
            end_time,
            order,
            value,
        });
    }

    pub(crate) fn pop(&mut self, now: Tick) -> Vec<T> {
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

    #[cfg(test)]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.q.len()
    }
}

struct ValueWrap<I: StableTypeId> {
    id: Id<I>,
    end_time: Tick,
}

impl<I: StableTypeId> ValueWrap<I> {
    fn new(id: Id<I>, end_time: Tick) -> Self {
        ValueWrap { id, end_time }
    }
}

impl<I: StableTypeId> EndTick for ValueWrap<I> {
    fn end_tick(&self, _: Tick) -> Tick {
        self.end_time
    }
}

/// This is a mutable version of TimeQueue. It allows to mutate the values in the queue.
/// It is a logical error to mutate the end_time of the value such that the order of the queue is changed.
/// TODO taxi driver needs to be able to change his end_time such that order is changed
pub(crate) struct MutTimeQueue<T, I>
where
    T: EndTick + Identifiable<I>,
    I: StableTypeId,
{
    q: TimeQueue<ValueWrap<I>, I>,
    cache: IntMap<Id<I>, T>,
}

impl<T, I> Default for MutTimeQueue<T, I>
where
    T: EndTick + Identifiable<I>,
    I: StableTypeId + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, I> MutTimeQueue<T, I>
where
    T: EndTick + Identifiable<I>,
    I: StableTypeId + 'static,
{
    pub(crate) fn new() -> Self {
        MutTimeQueue {
            q: TimeQueue::new(),
            cache: IntMap::default(),
        }
    }

    pub(crate) fn add(&mut self, value: T, now: Tick) {
        let id = value.id();
        self.q
            .add(ValueWrap::new(id.clone(), value.end_tick(now)), now);
        self.cache.insert(id.clone(), value);
    }

    pub(crate) fn pop(&mut self, now: Tick) -> Vec<T> {
        let ids = self.q.pop(now);
        let mut result: Vec<T> = Vec::new();

        for id in ids {
            let value = self.cache.remove(&id.id).unwrap();
            result.push(value);
        }

        result
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.cache.values_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestItem {
        id: u32,
        end: Tick,
    }

    impl EndTick for TestItem {
        fn end_tick(&self, _now: Tick) -> Tick {
            self.end
        }
    }

    #[test]
    fn test_time_queue_stable_ordering() {
        let mut queue: TimeQueue<TestItem, ()> = TimeQueue::new();
        queue.add(
            TestItem {
                id: 1,
                end: Tick::new(10),
            },
            Tick::zero(),
        );
        queue.add(
            TestItem {
                id: 2,
                end: Tick::new(10),
            },
            Tick::zero(),
        );
        queue.add(
            TestItem {
                id: 3,
                end: Tick::new(10),
            },
            Tick::zero(),
        );

        let results = queue.pop(Tick::new(10));
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, 1);
        assert_eq!(results[1].id, 2);
        assert_eq!(results[2].id, 3);
    }

    #[test]
    fn test_time_queue_time_ordering_priority() {
        let mut queue: TimeQueue<TestItem, ()> = TimeQueue::new();
        queue.add(
            TestItem {
                id: 1,
                end: Tick::new(15),
            },
            Tick::zero(),
        );
        queue.add(
            TestItem {
                id: 2,
                end: Tick::new(10),
            },
            Tick::zero(),
        );
        queue.add(
            TestItem {
                id: 3,
                end: Tick::new(20),
            },
            Tick::zero(),
        );
        queue.add(
            TestItem {
                id: 4,
                end: Tick::new(10),
            },
            Tick::zero(),
        );

        let results = queue.pop(Tick::new(10));
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, 2);
        assert_eq!(results[1].id, 4);

        let results = queue.pop(Tick::new(15));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);

        let results = queue.pop(Tick::new(20));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 3);
    }
}
