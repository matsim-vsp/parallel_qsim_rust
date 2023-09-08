use std::{collections::HashMap, hash::Hash, hash::Hasher, marker::PhantomData, rc::Rc};

pub type Id<T> = Rc<IdImpl<T>>;

#[derive(Debug, Ord, PartialOrd)]
pub struct IdImpl<T> {
    _type_marker: PhantomData<T>,
    pub internal: usize,
    pub external: String,
}

impl<T> IdImpl<T> {
    fn new(internal: usize, external: String) -> Id<T> {
        let id = IdImpl {
            internal,
            external,
            _type_marker: PhantomData,
        };
        Rc::new(id)
    }

    /// Creates an id which is not attached to any id storage. This method is intended for test
    /// cases. The intended way of creating ids is to use IdStore::create_id(external);
    pub fn new_internal(internal: usize) -> Id<T> {
        let id = IdImpl {
            internal,
            external: String::from(""),
            _type_marker: PhantomData,
        };
        Rc::new(id)
    }
}

impl<T> PartialEq for IdImpl<T> {
    fn eq(&self, other: &Self) -> bool {
        self.internal.eq(&other.internal)
    }
}

impl<T> Eq for IdImpl<T> {}

impl<T> Hash for IdImpl<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.internal.hash(state)
    }
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
        let id = IdImpl::new(next_internal, String::from(id));
        self.ids.push(id.clone());

        let ptr_external: *const String = &id.external;
        let external_ref = unsafe { ptr_external.as_ref() }.unwrap();
        self.mapping.insert(external_ref, id.internal);

        id
    }

    pub fn get(&self, internal: usize) -> Id<T> {
        self.ids
            .get(internal)
            .unwrap_or_else(|| panic!("No id found for internal {internal}"))
            .clone()
    }

    pub fn get_from_ext(&self, external: &str) -> Id<T> {
        let index = self
            .mapping
            .get(external)
            .unwrap_or_else(|| panic!("Could not find id for external id: {external}"));
        self.ids.get(*index).unwrap().clone()
    }
}

#[cfg(test)]
mod tets {
    use super::{Id, IdImpl, IdStore};

    #[test]
    fn test_id_eq() {
        let id: Id<()> = IdImpl::new(1, String::from("external-id"));
        assert_eq!(id, id.clone());

        let equal: Id<()> = IdImpl::new(
            1,
            String::from("other-external-value-which-should-be-ignored"),
        );
        assert_eq!(id, equal);

        let unequal: Id<()> = IdImpl::new(2, String::from("external-id"));
        assert_ne!(id, unequal)
    }

    #[test]
    fn id_store_create() {
        let mut store: IdStore<()> = IdStore::new();
        let external = String::from("external-id");

        let id = store.create_id(&external);

        assert_eq!(id.external, external)
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

        let fetched_1 = store.get(id_1.internal);
        let fetched_2 = store.get(id_2.internal);
        assert_eq!(fetched_1.external, external_1);
        assert_eq!(fetched_2.external, external_2);
    }

    #[test]
    fn id_store_get_ext() {
        let mut store: IdStore<()> = IdStore::new();
        let external_1 = String::from("id-1");
        let external_2 = String::from("id-2");
        let id_1 = store.create_id(&external_1);
        let id_2 = store.create_id(&external_2);

        assert_eq!(2, store.ids.len());
        assert_eq!(2, store.mapping.len());

        let fetched_1 = store.get_from_ext(&external_1);
        let fetched_2 = store.get_from_ext(&external_2);
        assert_eq!(fetched_1.external, external_1);
        assert_eq!(fetched_2.external, external_2);
    }
}
