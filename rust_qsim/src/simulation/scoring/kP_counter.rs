use crate::simulation::framework_events::{
    PartitionEvent, PartitionListenerRegisterFn, QSimId, RuntimeEvent,
};
use nohash_hasher::IntMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct KPCounter {
    output_path: PathBuf,
    rank: QSimId,

    partition_id2leaving_person_count: IntMap<QSimId, u32>,
    partition_id2leaving_vehicle_count: IntMap<QSimId, u32>,
}

impl KPCounter {
    pub fn new(output_path: PathBuf, rank: QSimId) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            output_path,
            rank,

            partition_id2leaving_person_count: IntMap::default(),
            partition_id2leaving_vehicle_count: IntMap::default(),
        }))
    }

    pub fn register_fn(counter: Arc<Mutex<KPCounter>>) -> Box<PartitionListenerRegisterFn> {
        Box::new(|events| {
            events.on_event(move |e: &RuntimeEvent<PartitionEvent>| match &e.payload {
                PartitionEvent::VehicleLeavesPartition(i) => {
                    let mut guard = counter.lock().unwrap();
                    *guard
                        .partition_id2leaving_vehicle_count
                        .entry(i.to)
                        .or_default() += 1
                }
                PartitionEvent::AgentLeavesPartition(i) => {
                    let mut guard = counter.lock().unwrap();
                    *guard
                        .partition_id2leaving_person_count
                        .entry(i.to)
                        .or_default() += 1
                }
                _ => {}
            });
        })
    }

    pub fn finish(&self) {
        let mut o = self.output_path.clone();
        o.push("counts");
        fs::create_dir_all(&o).expect("Failed to create counts directory");
        o.push(format!("output_partition_counts_{}.csv", self.rank));

        let file = File::create(&o).expect("Failed to create output file");
        let mut w = BufWriter::new(file);
        writeln!(w, "type,target_partition,count").unwrap();
        let mut persons: Vec<_> = self.partition_id2leaving_person_count.iter().collect();
        persons.sort_by_key(|(k, _)| *k);
        for (partition, count) in persons {
            writeln!(w, "person,{},{}", partition, count).unwrap();
        }
        let mut vehicles: Vec<_> = self.partition_id2leaving_vehicle_count.iter().collect();
        vehicles.sort_by_key(|(k, _)| *k);
        for (partition, count) in vehicles {
            writeln!(w, "vehicle,{},{}", partition, count).unwrap();
        }
    }
}
