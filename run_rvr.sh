NUM_PARTS="$1"

export RUST_BACKTRACE=1
./target/release/partition_network\
 --in-path /Users/janek/Documents/rust_q_sim/input/rvr.network.xml.gz\
 --out-path /Users/janek/Documents/rust_q_sim/output-"$NUM_PARTS"/rvr.network."$NUM_PARTS".xml.gz\
 --num-parts "$NUM_PARTS"

mpirun --np "$NUM_PARTS" ./target/release/mpi_qsim\
  --network-file /Users/janek/Documents/rust_q_sim/output-"$NUM_PARTS"/rvr.network."$NUM_PARTS".xml.gz\
  --population-file /Users/janek/Documents/rust_q_sim/input/rvr.plans.xml.gz\
  --vehicles-file /Users/janek/Documents/rust_q_sim/input/rvr.vehicles.xml\
  --output-dir /Users/janek/Documents/rust_q_sim/output-"$NUM_PARTS"\
  --partition-method none\
  --num-parts "$NUM_PARTS"

./target/release/merge_xml_events\
 --path /Users/janek/Documents/rust_q_sim/output-"$NUM_PARTS"\
  --num-parts "$NUM_PARTS"
