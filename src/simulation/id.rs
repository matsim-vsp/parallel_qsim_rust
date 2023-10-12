use std::cmp::Ordering;
use std::slice::Iter;
use std::{collections::HashMap, hash::Hash, hash::Hasher, marker::PhantomData, rc::Rc};

/// This type represents a reference counted pointer to a matsim id. It can be used in hash maps/sets
/// in combination with NoHashHasher, to achieve fast look ups with no randomness involved.
///
/// As this type wraps Rc<IdImpl<T>>, using clone produces a new Rc pointer to the actual Id and is
/// the intended way of passing around ids.  
///
/// This type is intended to be used in combination with an [IdStore], which will take care of assigning
/// unique internal ids. To create a new id use [IdStore::create_id()]
///
/// This type uses the newtype pattern https://rust-unofficial.github.io/patterns/patterns/behavioural/newtype.html
/// to hide internal representation and to enable implementing IsEnabled for using the NoHashHasher create
#[derive(Debug)]
pub struct Id<T>(Rc<IdImpl<T>>);

impl<T> Id<T> {
    fn new(internal: usize, external: String) -> Self {
        Self(Rc::new(IdImpl {
            _type_marker: PhantomData,
            internal,
            external,
        }))
    }

    /// Creates an id which is not attached to any id storage. This method is intended for test
    /// cases. The intended way of creating ids is to use IdStore::create_id(external);
    #[cfg(test)]
    pub(crate) fn new_internal(internal: usize) -> Self {
        Self::new(internal, String::from(""))
    }

    pub fn internal(&self) -> usize {
        self.0.internal
    }

    pub fn external(&self) -> &str {
        &self.0.external
    }
}

/// Mark Id as enabled for the nohash_hasher::NoHashHasher trait
impl<T> nohash_hasher::IsEnabled for Id<T> {}

impl<T> nohash_hasher::IsEnabled for &Id<T> {}

/// Implement PartialEq, Eq, PartialOrd, Ord, so that Ids can be used in HashMaps and Ordered collections
/// all four methods rely on the internal id.
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.internal().eq(&other.internal())
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // use write usize directly, so that we can use NoHashHasher with ids
        state.write_usize(self.internal());
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.internal().cmp(&other.internal())
    }
}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.internal().partial_cmp(&other.internal())
    }
}

/// This creates a new struct with a cloned Rc pointer
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Debug)]
pub struct IdImpl<T> {
    _type_marker: PhantomData<T>,
    pub internal: usize,
    pub external: String,
}

#[derive(Debug, Default)]
pub struct IdStore<'ext, T> {
    ids: Vec<Id<T>>,
    mapping: HashMap<&'ext str, usize>,
}

impl<'ext, T> IdStore<'ext, T> {
    pub fn new() -> Self {
        IdStore {
            ids: Vec::new(),
            mapping: HashMap::new(),
        }
    }

    /// creates a new id if not yet present for the passed external id
    /// in case no id was present a new String is allocated and associated
    /// with the returned id.
    /// In case of an Id already present for the passed id parameter, a
    /// reference to that id is returned
    pub fn create_id(&mut self, id: &str) -> Id<T> {
        if self.mapping.contains_key(id) {
            let index = *self.mapping.get(id).unwrap();
            return self.ids.get(index).unwrap().clone();
        }

        // no id yet. create one
        let next_internal = self.ids.len();
        let id = Id::new(next_internal, String::from(id));
        self.ids.push(id.clone());

        let ptr_external: *const String = &id.0.external;

        /*
        # Safety:

        As the external Strings are allocated by the ids, which keep a pointer to that allocation
        The allocated string will not move as long as the id exists. This means as long as the id
        is in the map, the ref to the external String which is used as a key in the map will be valid
         */
        let external_ref = unsafe { ptr_external.as_ref() }.unwrap();
        self.mapping.insert(external_ref, id.internal());

        id
    }

    pub fn get(&self, internal: usize) -> Id<T> {
        self.ids
            .get(internal)
            .unwrap_or_else(|| panic!("No id found for internal {internal}"))
            .clone()
    }

    pub fn get_from_wire(&self, internal: u64) -> Id<T> {
        self.get(internal as usize)
    }

    pub fn get_from_ext(&self, external: &str) -> Id<T> {
        let index = self
            .mapping
            .get(external)
            .unwrap_or_else(|| panic!("Could not find id for external id: {external}"));
        self.ids.get(*index).unwrap().clone()
    }

    pub fn exists(&self, id: Id<T>) -> bool {
        self.ids.get(id.internal()).is_some()
    }

    pub fn iter(&self) -> Iter<'_, Id<T>> {
        self.ids.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::{Id, IdStore};

    #[test]
    fn test_id_eq() {
        let id: Id<()> = Id::new(1, String::from("external-id"));
        assert_eq!(id, id.clone());

        let equal: Id<()> = Id::new(
            1,
            String::from("other-external-value-which-should-be-ignored"),
        );
        assert_eq!(id, equal);

        let unequal: Id<()> = Id::new(2, String::from("external-id"));
        assert_ne!(id, unequal)
    }

    #[test]
    fn id_store_create() {
        let mut store: IdStore<()> = IdStore::new();
        let external = String::from("external-id");

        let id = store.create_id(&external);

        assert_eq!(id.external(), external)
    }

    #[test]
    fn id_store_get() {
        let mut store: IdStore<()> = IdStore::new();
        let external_1 = String::from("id-1");
        let external_2 = String::from("id-2");
        let id_1 = store.create_id(&external_1);
        let id_2 = store.create_id(&external_2);

        assert_eq!(2, store.ids.len());
        assert_eq!(2, store.mapping.len());

        let fetched_1 = store.get(id_1.internal());
        let fetched_2 = store.get(id_2.internal());
        assert_eq!(fetched_1.external(), external_1);
        assert_eq!(fetched_2.external(), external_2);
    }

    #[test]
    fn id_store_get_ext() {
        let mut store: IdStore<()> = IdStore::new();
        let external_1 = String::from("id-1");
        let external_2 = String::from("id-2");
        let _id_1 = store.create_id(&external_1);
        let _id_2 = store.create_id(&external_2);

        assert_eq!(2, store.ids.len());
        assert_eq!(2, store.mapping.len());

        let fetched_1 = store.get_from_ext(&external_1);
        let fetched_2 = store.get_from_ext(&external_2);
        assert_eq!(fetched_1.external(), external_1);
        assert_eq!(fetched_2.external(), external_2);
    }
}
