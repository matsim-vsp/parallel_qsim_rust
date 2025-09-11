use crate::simulation::id::id_store::IdStore;
use crate::simulation::id::id_store::UntypedId;
use crate::simulation::id::serializable_type::StableTypeId;
use once_cell::sync::Lazy;
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

// keep this private, as we don't want to leak how we cache ids.
mod id_store;
pub mod serializable_type;

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
pub struct Id<T: StableTypeId> {
    _type_marker: PhantomData<T>,
    id: Arc<UntypedId>,
}

impl<T: StableTypeId + 'static> Id<T> {
    fn new(untyped_id: Arc<UntypedId>) -> Self {
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
        Self::new(Arc::new(untyped_id))
    }

    pub fn internal(&self) -> u64 {
        self.id.internal
    }

    pub fn external(&self) -> &str {
        &self.id.external
    }

    pub fn create(id: &str) -> Self {
        ID_STORE.create_id(id)
    }

    pub fn get(internal: u64) -> Self {
        ID_STORE.get(internal)
    }

    pub fn get_from_ext(external: &str) -> Self {
        ID_STORE.get_from_ext(external)
    }

    pub fn try_get_from_ext(external: &str) -> Option<Self> {
        ID_STORE.try_get_from_ext(external)
    }
}

pub fn store_to_file(file_path: &Path) {
    ID_STORE.to_file(file_path)
}

pub fn load_from_file(file_path: &Path) {
    ID_STORE.load_from_file(file_path)
}

/// Mark Id as enabled for the nohash_hasher::NoHashHasher t
impl<T: StableTypeId> nohash_hasher::IsEnabled for Id<T> {}
impl<T: StableTypeId> nohash_hasher::IsEnabled for &Id<T> {}

/// Implement PartialEq, Eq, PartialOrd, Ord, so that Ids can be used in HashMaps and Ordered collections
/// all four methods rely on the internal id.
impl<T: StableTypeId + 'static> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.internal().eq(&other.internal())
    }
}

impl<T: StableTypeId + 'static> Eq for Id<T> {}

impl<T: StableTypeId + 'static> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // use write u64 directly, so that we can use NoHashHasher with ids
        state.write_u64(self.internal());
    }
}

impl<T: StableTypeId + 'static> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.internal().cmp(&other.internal())
    }
}

impl<T: StableTypeId + 'static> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: StableTypeId + 'static> Display for Id<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.external())
    }
}

/// This creates a new struct with a cloned Rc pointer
impl<T: StableTypeId> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self {
            _type_marker: PhantomData,
            id: self.id.clone(),
        }
    }
}

// Replace the thread_local store with a static concurrent store
static ID_STORE: Lazy<IdStore> = Lazy::new(IdStore::new);

#[derive(Debug, Error)]
pub enum IdError {
    #[error("Id with internal value {0} not found")]
    NotFound(u64),
    #[error("Id with external value {0} not found")]
    NotFoundExt(String),
}

#[cfg(test)]
mod tests {
    use crate::simulation::id::{Id, UntypedId};
    use std::sync::Arc;

    #[test]
    fn test_id_eq() {
        let id: Id<()> = Id::new(Arc::new(UntypedId::new(1, String::from("external-id"))));
        assert_eq!(id, id.clone());

        let equal = Id::new(Arc::new(UntypedId::new(
            1,
            String::from("other-external-value-which-should-be-ignored"),
        )));
        assert_eq!(id, equal);

        let unequal = Id::new(Arc::new(UntypedId::new(2, String::from("external-id"))));
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
