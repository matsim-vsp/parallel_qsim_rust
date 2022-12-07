use metis::{Graph, Idx};

#[derive(Debug, Eq, PartialEq)]
struct Node {
    id: usize,
    out_links: Vec<usize>,
    in_links: Vec<usize>,
    weight: i32,
}

#[derive(Debug)]
struct Link {
    id: usize,
    count: i32,
    to: usize,
    from: usize,
}

fn partition(nodes: Vec<Node>, links: Vec<Link>) -> Vec<Idx> {
    let mut xadj: Vec<Idx> = Vec::from([0]);
    let mut adjncy: Vec<Idx> = Vec::new();
    let mut adjwgt: Vec<Idx> = Vec::new();
    let mut vwgt: Vec<Idx> = Vec::new();
    let mut node_ids: Vec<usize> = Vec::new();

    for node in nodes {
        // do the xadj  pointers.
        let number_of_out_links = node.out_links.len() as Idx;
        let number_of_in_links = node.in_links.len() as Idx;
        let next_adjacency_index = xadj.last().unwrap() + number_of_out_links + number_of_in_links;
        xadj.push(next_adjacency_index);
        vwgt.push(node.weight as Idx);
        node_ids.push(node.id);

        // write the adjacent nodes and the link weights
        for link_id in node.out_links {
            let link = links.get(link_id).unwrap();
            adjncy.push(link.to as Idx);
            adjwgt.push(link.count as Idx);
        }

        for link_id in node.in_links {
            let link = links.get(link_id).unwrap();
            adjncy.push(link.from as Idx);
            adjwgt.push(link.count as Idx);
        }
    }

    let mut result: Vec<Idx> = vec![0x00; xadj.len() - 1];

    println!("{:?}", xadj);
    println!("{:?}", adjncy);
    println!("{:?}", adjwgt);
    println!("{:?}", vwgt);
    println!("{:?}", node_ids);

    // ncon specifies number of vertice weights. Our vertices are unweighted
    // nparts is the number of parts. We'll start with 2
    Graph::new(1, 2, &mut xadj, &mut adjncy)
        .set_adjwgt(&mut adjwgt)
        .set_vwgt(&mut vwgt)
        .part_kway(&mut result)
        .unwrap();

    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use metis::{Graph, Idx};

    use crate::experiments::metis_test::{partition, Link, Node};
    use crate::io::network::IONetwork;

    #[test]
    fn test_convert_example() {
        let (nodes, links) = create_network();

        let result = partition(nodes, links);

        let mut expected: Vec<Idx> = vec![0x00; 15];
        let mut crs = create_example();
        Graph::new(1, 2, &mut crs.0, &mut crs.1)
            .part_kway(&mut expected)
            .unwrap();

        // this assertion basically tests whether the partition method converts the node/link structure
        // into the crs format correctly.
        assert_eq!(expected, result)
    }

    #[test]
    fn test_convert_vertice_weight() {
        let network = IONetwork::from_file("./assets/3-links/3-links-network.xml");
        let mut nodes: HashMap<_, _> = network
            .nodes()
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let weight = if i == 1 || i == 2 { 100 } else { 0 };
                let result = Node {
                    id: i,
                    weight,
                    out_links: Vec::new(),
                    in_links: Vec::new(),
                };
                (node.id.as_str(), result)
            })
            .collect();

        let links: Vec<_> = network
            .links()
            .iter()
            .enumerate()
            .map(|(i, link)| {
                // add the link to its from node
                let from_node = nodes.get_mut(&link.from.as_str()).unwrap();
                let from_id = from_node.id;
                from_node.out_links.push(i);
                let to_node = nodes.get_mut(&link.to.as_str()).unwrap();
                let to_id = to_node.id;
                to_node.in_links.push(i);

                Link {
                    id: i,
                    to: to_id,
                    from: from_id,
                    count: 1,
                }
            })
            .collect();

        let mut vec: Vec<_> = nodes.into_iter().map(|(_, node)| node).collect();
        vec.sort_by(|a, b| a.id.cmp(&b.id));
        let result = partition(vec, links);

        println!("{:?} this is the result", result);
        assert_eq!(vec![1, 1, 0, 0], result);
    }

    #[rustfmt::skip]
    fn create_network() -> (Vec<Node>, Vec<Link>) {
        let (xadj, adjncy) = create_example();
        let mut links = Vec::with_capacity(adjncy.len());

        let mut nodes : Vec<Node> = xadj.iter().enumerate()
            .filter(|(i, _index) | *i < xadj.len() - 1)
            .map(|(i, _index)| { Node {
                id: i,
                weight: 1,
                out_links: Vec::new(),
                in_links: Vec::new(),
            }})
            .collect();

        for from_id in 0..xadj.len() - 1 {
            let start_i = xadj[from_id];
            let end_i = xadj[from_id + 1];
            //let mut out_links = Vec::new();

            for l in start_i..end_i {
                let to_id: Idx = adjncy[l as usize];
                let link = Link {
                    id: links.len(),
                    count: 1,
                    to: to_id as usize,
                    from: from_id,
                };

                // wire up the from and to node for this link
                let from_node = nodes.get_mut(from_id).unwrap();
                from_node.out_links.push(link.id);
                let to_node = nodes.get_mut(to_id as usize).unwrap();
                to_node.in_links.push(link.id);

                // move link into link list
                links.push(link);
            }
        }

        println!("{:?}", nodes);
        println!("{:?}", links);
        (nodes, links)
    }

    /// This takes an example from https://github.com/LIHPC-Computational-Geometry/metis-rs/blob/master/examples/graph.rs
    /// Which itself is the example from https://raw.githubusercontent.com/KarypisLab/METIS/master/manual/manual.pdf
    /// chapter 5.5 - Figure 3
    #[rustfmt::skip]
    fn create_example() -> ([Idx; 16], [Idx; 44]) {
        let xadj = [0, 2, 5, 8, 11, 13, 16, 20, 24, 28, 31, 33, 36, 39, 42, 44];
        let adjncy = [
            1, 5,
            0, 2, 6,
            1, 3, 7,
            2, 4, 8,
            3, 9,
            0, 6, 10,
            1, 5, 7, 11,
            2, 6, 8, 12,
            3, 7, 9, 13,
            4, 8, 14,
            5, 11,
            6, 10, 12,
            7, 11, 13,
            8, 12, 14,
            9, 13,
        ];
        println!("{:?}", xadj);
        println!("{:?}", adjncy);
        (xadj, adjncy)
    }
}
