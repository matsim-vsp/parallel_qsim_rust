use std::{collections::HashMap, marker::PhantomData, rc::Rc};

type Id<T> = Rc<IdImpl<T>>;

#[derive(Debug)]
pub struct IdImpl<T> {
    _type_marker: PhantomData<T>,
    internal: u64,
    external: String,
}

impl<T> IdImpl<T> {
    fn new(internal: u64, external: String) -> Id<T> {
        let id = IdImpl {
            internal,
            external,
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

pub struct IdStore<'ext, T> {
    ids: Vec<Id<T>>,
    mapping: HashMap<&'ext str, u64>,
}

impl<'ext, T> IdStore<'ext, T> {
    pub fn new() -> Self {
        IdStore {
            ids: Vec::new(),
            mapping: HashMap::new(),
        }
    }

    pub fn create_id(&mut self, id: String) -> Id<T> {
        if self.mapping.contains_key(id.as_str()) {
            let index = *self.mapping.get(&id.as_str()).unwrap();
            return self.ids.get(index as usize).unwrap().clone();
        }

        // no id yet. create one
        let next_internal = self.ids.len();
        let id = IdImpl::new(next_internal as u64, id);
        self.ids.push(id.clone());

        let ptr_external: *const String = &id.external;
        let external_ref = unsafe { ptr_external.as_ref() }.unwrap();
        self.mapping.insert(external_ref, id.internal);

        id
    }

    pub fn get(&self, internal: u64) -> Id<T> {
        self.ids.get(internal as usize).unwrap().clone()
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

        let id = store.create_id(external.clone());

        assert_eq!(id.external, external)
    }

    #[test]
    fn id_store_get() {
        
        let mut store: IdStore<()> = IdStore::new();
        let external_1 = String::from("id-1");
        let external_2 = String::from("id-2");
        let id_1 = store.create_id(external_1.clone());
        let id_2 = store.create_id(external_2.clone());
        
        assert_eq!(2, store.ids.len());
        assert_eq!(2, store.mapping.len());
        
        let fetched_1 = store.get(id_1.internal);
        let fetched_2 = store.get(id_2.internal);
        assert_eq!(fetched_1.external, external_1);
        assert_eq!(fetched_2.external, external_2);
    }
}
