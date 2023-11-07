NUM_PARTS="$1"

export RUST_BACKTRACE=1

cargo run --release --bin partition_network --\
 --in-path /Users/janek/Documents/rust_q_sim/input/rvr.network.xml.gz\
 --num-parts "$NUM_PARTS"

cargo mpirun --np "$NUM_PARTS" --release --bin mpi_qsim --\
  --network-file /Users/janek/Documents/rust_q_sim/input/rvr.network."$NUM_PARTS".xml.gz\
  --population-file /Users/janek/Documents/rust_q_sim/input/rvr-single.plans.xml.gz\
  --vehicles-file /Users/janek/Documents/rust_q_sim/input/rvr.vehicles.xml\
  --output-dir /Users/janek/Documents/rust_q_sim/output-"$NUM_PARTS"\
  --partition-method none\
  --num-parts "$NUM_PARTS"

cargo run --release --bin proto2xml --\
 --path /Users/janek/Documents/rust_q_sim/output-"$NUM_PARTS"/\
 --num-parts "$NUM_PARTS"
