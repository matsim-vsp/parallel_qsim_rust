use std::cell::RefCell;
use std::rc::Rc;
use crate::simulation::framework_events::QSimId;
use crate::simulation::scoring::homesending::homesending_data_collector::HomeSendingDataCollector;
use crate::simulation::scoring::homesending::homesending_message_broker::HomeSendingMessageBroker;

pub struct HomesendingScoringEngine
{
    homesending_data_collector: Rc<RefCell<HomeSendingDataCollector>>,
    homesending_message_broker: Rc<RefCell<HomeSendingMessageBroker>>,
    rank: QSimId
}
