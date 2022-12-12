use log::info;
use std::collections::HashMap;
use std::sync::mpsc::Receiver;

pub struct SingleMessageLogger {
    receiver: Receiver<LogMessage>,
    num_parts: usize,
    number_of_shutdown_signals: usize,
    received_messages: HashMap<String, usize>,
}

pub enum LogMessage {
    Message(String),
    Shutdown,
}

impl SingleMessageLogger {
    pub fn new(receiver: Receiver<LogMessage>, num_parts: usize) -> SingleMessageLogger {
        SingleMessageLogger {
            received_messages: HashMap::new(),
            receiver,
            num_parts,
            number_of_shutdown_signals: 0,
        }
    }
    pub fn recv(&mut self) {
        loop {
            let message = self.receiver.recv().unwrap();
            match message {
                LogMessage::Message(text) => {
                    let times = self
                        .received_messages
                        .entry(text.clone())
                        .and_modify(|v| *v += 1)
                        .or_insert(1);

                    if *times == self.num_parts {
                        self.received_messages.remove(&text).unwrap();
                        info!("{}", text);
                    }
                }
                LogMessage::Shutdown => {
                    if self.number_of_shutdown_signals == self.num_parts - 1 {
                        info!("received shutdown, so stop waiting.");
                        break;
                    } else {
                        self.number_of_shutdown_signals += 1;
                    }
                }
            }
        }
    }
}
