use core::any::Any;
use rust_qsim::simulation::events::{EventTrait, LinkEnterEvent, LinkLeaveEvent};
use rust_qsim::simulation::id::Id;
use std::ops::Deref;

trait DynEq: Any {
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, arg1: &dyn DynEq) -> bool;
}

impl<T: Any + PartialEq> DynEq for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, arg1: &dyn DynEq) -> bool {
        if let Some(other) = arg1.as_any().downcast_ref::<Self>() {
            self == other
        } else {
            false
        }
    }
}

impl PartialEq for dyn DynEq {
    fn eq(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

fn main() {
    // link enter event
    let le: Box<dyn EventTrait> = Box::new(LinkEnterEvent {
        time: 0,
        link: Id::create("link"),
        vehicle: Id::create("link"),
        attributes: Default::default(),
    });
    // link leave event
    let ll: Box<dyn EventTrait> = Box::new(LinkLeaveEvent {
        time: 0,
        link: Id::create("link"),
        vehicle: Id::create("link"),
        attributes: Default::default(),
    });

    let le_ref = le.deref();
    let ll_ref = ll.deref();

    // check if the two events are equal (should be false as they are of different types)
    dbg!(le_ref == ll_ref);

    // sanity check if the event is equal to itself (should be true)
    dbg!(le_ref == le_ref);

    // ===========

    // checks on more simple types
    let x: &dyn DynEq = &42u32;

    let y = String::from("foo");
    let y: &dyn DynEq = &y;

    let z = String::from("bar");
    let z: &dyn DynEq = &z;

    dbg!(x == x);
    dbg!(x == y);
    dbg!(x == z);

    dbg!(y == x);
    dbg!(y == y);
    dbg!(y == z);

    dbg!(z == x);
    dbg!(z == y);
    dbg!(z == z);
}
