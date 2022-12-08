import subprocess
import sys
import os

def convert_network_into_binary(inertial_flow_path, data_path, file):
    
    args = [inertial_flow_path + "/build/console"]
    args.append("text_to_binary_vector")
    args.append(data_path + "/" + file)

    if not os.path.exists(data_path + "/binary"):
        os.mkdir(data_path + "/binary")

    args.append(data_path + "/binary/" + file)

    call_subprocess(args)
    
def compute_ordering(inertial_flow_path, data_path, file):
    args = ["python3"]
    args.append(inertial_flow_path +  "/inertialflowcutter_order.py")
    args.append(data_path + "/binary/")

    if not os.path.exists(data_path + "/ordering"):
        os.mkdir(data_path + "/ordering")
        
    args.append(data_path + "/ordering/" + file + "_bin")

    call_subprocess(args)

def convert_ordering_into_text(inertial_flow_path, data_path, file):
    args = [inertial_flow_path + "/build/console"]
    args.append("binary_to_text_vector")
    args.append(data_path + "/ordering/" + file + "_bin")
    args.append(data_path + "/ordering/" + file)

    call_subprocess(args)

def call_subprocess(args):
    print("Call process: ", *args)
    subprocess.run(args)

if __name__ == '__main__':
    # phase 1.2: convert RoutingKit format into binary
    for f in ["head", "travel_time", "first_out", "latitude", "longitude"]:
        convert_network_into_binary(sys.argv[1], sys.argv[2], f)
    # phase 1.3: compute ordering
    compute_ordering(sys.argv[1], sys.argv[2], sys.argv[3])
    # phase 1.4: convert ordering into text
    convert_ordering_into_text(sys.argv[1], sys.argv[2], sys.argv[3])
