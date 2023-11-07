use std::any::TypeId;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::rc::Rc;

use ahash::{AHashMap, RandomState};

/// This type represents a reference counted pointer to a matsim id. It can be used in hash maps/sets
/// in combination with NoHashHasher, to achieve fast look ups with no randomness involved.
///
/// As this type wraps Rc<IdImpl<T>>, using clone produces a new Rc pointer to the actual Id and is
/// the intended way of passing around ids.
///
/// This type uses the newtype pattern https://rust-unofficial.github.io/patterns/patterns/behavioural/newtype.html
/// to hide internal representation and to enable implementing IsEnabled for using the NoHashHasher create
/// Also, it uses the new type pattern because we wrap an untyped id, so that we can have a global id store of all
/// ids.
#[derive(Debug)]
pub struct Id<T> {
    _type_marker: PhantomData<T>,
    id: Rc<UntypedId>,
}

pub const STRING_TYPE_ID: u64 = 1;

impl<T: 'static> Id<T> {
    fn new(untyped_id: Rc<UntypedId>) -> Self {
        Self {
            _type_marker: PhantomData,
            id: untyped_id,
        }
    }

    /// Creates an id which is not attached to any id storage. This method is intended for test
    /// cases. The intended way of creating ids is to use IdStore::create_id(external);
    #[cfg(test)]
    pub(crate) fn new_internal(internal: u64) -> Self {
        let untyped_id = UntypedId::new(internal, String::from(""));
        Self::new(Rc::new(untyped_id))
    }

    pub fn internal(&self) -> u64 {
        self.id.internal
    }

    pub fn external(&self) -> &str {
        &self.id.external
    }

    pub fn create(id: &str) -> Self {
        ID_STORE.with(|store| store.borrow_mut().create_id(id))
    }

    pub fn get(internal: u64) -> Self {
        ID_STORE.with(|store| store.borrow().get(internal))
    }

    pub fn get_from_ext(external: &str) -> Self {
        ID_STORE.with(|store| store.borrow().get_from_ext(external))
    }
}

/// Mark Id as enabled for the nohash_hasher::NoHashHasher trait
impl<T> nohash_hasher::IsEnabled for Id<T> {}

impl<T> nohash_hasher::IsEnabled for &Id<T> {}

/// Implement PartialEq, Eq, PartialOrd, Ord, so that Ids can be used in HashMaps and Ordered collections
/// all four methods rely on the internal id.
impl<T: 'static> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.internal().eq(&other.internal())
    }
}

impl<T: 'static> Eq for Id<T> {}

impl<T: 'static> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // use write u64 directly, so that we can use NoHashHasher with ids
        state.write_u64(self.internal());
    }
}

impl<T: 'static> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.internal().cmp(&other.internal())
    }
}

impl<T: 'static> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.internal().partial_cmp(&other.internal())
    }
}

/// This creates a new struct with a cloned Rc pointer
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self {
            _type_marker: PhantomData,
            id: self.id.clone(),
        }
    }
}

thread_local! {static ID_STORE: RefCell<IdStore<'static>> = RefCell::new(IdStore::new())}

#[derive(Debug)]
struct UntypedId {
    internal: u64,
    external: String,
}

impl UntypedId {
    fn new(internal: u64, external: String) -> Self {
        Self { internal, external }
    }
}

#[derive(Debug)]
struct IdStore<'ext> {
    ids: AHashMap<TypeId, Vec<Rc<UntypedId>>>,
    // use ahasher algorithm with fixed random state, to get predictable
    mapping: AHashMap<TypeId, AHashMap<&'ext str, u64>>,
}

impl<'ext> IdStore<'ext> {
    fn new() -> Self {
        Self {
            ids: AHashMap::with_hasher(RandomState::with_seed(42)),
            mapping: AHashMap::with_hasher(RandomState::with_seed(42)),
        }
    }

    fn create_id<T: 'static>(&mut self, id: &str) -> Id<T> {
        let type_id = TypeId::of::<T>();

        let type_mapping = self
            .mapping
            .entry(type_id)
            .or_insert_with(|| AHashMap::with_hasher(RandomState::with_seed(42)));

        if type_mapping.contains_key(id) {
            return self.get_from_ext::<T>(id);
        }

        let type_ids = self.ids.entry(type_id).or_insert_with(Vec::default);
        let next_internal = type_ids.len() as u64;
        let next_id = Rc::new(UntypedId::new(next_internal, String::from(id)));
        type_ids.push(next_id.clone());

        let ptr_external: *const String = &next_id.external;
        /*
        # Safety:

        As the external Strings are allocated by the ids, which keep a pointer to that allocation
        The allocated string will not move as long as the id exists. This means as long as the id
        is in the map, the ref to the external String which is used as a key in the map will be valid
         */
        let external_ref = unsafe { ptr_external.as_ref() }.unwrap();
        type_mapping.insert(external_ref, next_id.internal);

        Id::new(next_id)
    }

    fn get<T: 'static>(&self, internal: u64) -> Id<T> {
        let type_id = TypeId::of::<T>();
        let type_ids = self.ids.get(&type_id).unwrap_or_else(|| {
            panic!("No ids for type {type_id:?}. Use Id::create::<T>(...) to create ids")
        });

        let untyped_id = type_ids
            .get(internal as usize)
            .unwrap_or_else(|| panic!("No id found for internal {internal}"))
            .clone();
        Id::new(untyped_id)
    }

    fn get_from_ext<T: 'static>(&self, external: &str) -> Id<T> {
        let type_id = TypeId::of::<T>();
        let type_mapping = self.mapping.get(&type_id).unwrap_or_else(|| {
            panic!("No ids for type {type_id:?}. Use Id::create::<T>(...) to create ids")
        });

        let index = type_mapping.get(external).unwrap_or_else(|| {
            panic!("Could not find id for external id: {external}");
        });

        self.get(*index)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::simulation::id::{Id, UntypedId};

    #[test]
    fn test_id_eq() {
        let id: Id<()> = Id::new(Rc::new(UntypedId::new(1, String::from("external-id"))));
        assert_eq!(id, id.clone());

        let equal = Id::new(Rc::new(UntypedId::new(
            1,
            String::from("other-external-value-which-should-be-ignored"),
        )));
        assert_eq!(id, equal);

        let unequal = Id::new(Rc::new(UntypedId::new(2, String::from("external-id"))));
        assert_ne!(id, unequal)
    }

    #[test]
    fn create_id() {
        let external = String::from("external-id");

        let id: Id<()> = Id::create(&external);
        assert_eq!(external, id.external());
        assert_eq!(0, id.internal());
    }

    #[test]
    fn create_id_duplicate() {
        let external = String::from("external-id");

        let id: Id<()> = Id::create(&external);
        let duplicate: Id<()> = Id::create(&external);

        assert_eq!(id, duplicate);
    }

    #[test]
    fn create_id_multiple_types() {
        let external = String::from("external-id");

        let int_id: Id<u32> = Id::create(&external);
        assert_eq!(external, int_id.external());
        assert_eq!(0, int_id.internal());

        let float_id: Id<f32> = Id::create(&external);
        assert_eq!(external, float_id.external());
        assert_eq!(0, float_id.internal());
    }

    #[test]
    fn get_id() {
        let external_1 = String::from("id-1");
        let external_2 = String::from("id-2");
        let id_1: Id<()> = Id::create(&external_1);
        let id_2: Id<()> = Id::create(&external_2);

        let fetched_1: Id<()> = Id::get(id_1.internal());
        let fetched_2: Id<()> = Id::get(id_2.internal());
        assert_eq!(fetched_1.external(), external_1);
        assert_eq!(fetched_2.external(), external_2);
    }

    #[test]
    fn id_store_get_ext() {
        let external_1 = String::from("id-1");
        let external_2 = String::from("id-2");
        let id_1: Id<()> = Id::create(&external_1);
        let id_2: Id<()> = Id::create(&external_2);

        let fetched_1: Id<()> = Id::get_from_ext(id_1.external());
        let fetched_2: Id<()> = Id::get_from_ext(id_2.external());
        assert_eq!(fetched_1.external(), external_1);
        assert_eq!(fetched_2.external(), external_2);
    }
}
