use metis::{Graph, Idx};

#[derive(Debug)]
struct Node {
    id: usize,
    out_links: Vec<usize>,
    weight: i32,
}

#[derive(Debug)]
struct Link {
    id: usize,
    count: i32,
    to: usize,
}

fn partition(nodes: Vec<Node>, links: Vec<Link>) -> Vec<Idx> {
    let mut xadj: Vec<Idx> = Vec::from([0]);
    let mut adjncy: Vec<Idx> = Vec::new();
    let mut adjwgt: Vec<Idx> = Vec::new();
    let mut vwgt: Vec<Idx> = Vec::new();
    let mut node_ids: Vec<usize> = Vec::new();

    for node in nodes {
        // do the xadj  pointers.
        let number_of_out_links = node.out_links.len() as i32;
        let next_adjacency_index = xadj.last().unwrap() + number_of_out_links;
        xadj.push(next_adjacency_index);
        vwgt.push(node.weight);
        node_ids.push(node.id);

        // write the adjacent nodes and the link weights
        for link_id in node.out_links {
            let link = links.get(link_id).unwrap();
            adjncy.push(link.to as i32);
            adjwgt.push(link.count);
        }
    }

    let mut result = vec![0x00; xadj.len() - 1];

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

    use metis::Graph;

    use crate::experiments::metis_test::{partition, Link, Node};
    use crate::io::network::IONetwork;

    #[test]
    fn test_convert_example() {
        let (nodes, links) = create_network();

        let result = partition(nodes, links);

        let mut expected = vec![0x00; 15];
        let mut crs = create_example();
        Graph::new(1, 2, &mut crs.0, &mut crs.1)
            .part_kway(&mut expected)
            .unwrap();

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
                from_node.out_links.push(i);

                let count = if i == 1 { 25 } else { 0 };

                // create the link
                let to_node = nodes.get(&link.to.as_str()).unwrap();
                Link {
                    id: i,
                    to: to_node.id,
                    count,
                }
            })
            .collect();

        let mut vec: Vec<_> = nodes.into_iter().map(|(_, node)| node).collect();
        vec.sort_by(|a, b| a.id.cmp(&b.id));
        let result = partition(vec, links);

        println!("{:?} this is the result", result);
    }

    #[rustfmt::skip]
    fn create_network() -> (Vec<Node>, Vec<Link>) {
        let (xadj, adjncy) = create_example();
        let mut nodes = Vec::with_capacity(xadj.len() - 1);
        let mut links = Vec::with_capacity(adjncy.len());

        for i in 0..xadj.len() - 1 {
            let start_i = xadj[i];
            let end_i = xadj[i + 1];
            let mut out_links = Vec::new();

            for l in start_i..end_i {
                let node_id: i32 = adjncy[l as usize];
                let link = Link {
                    id: links.len(),
                    count: 1,
                    to: node_id as usize,
                };
                out_links.push(link.id);
                links.push(link);
            }

            let id = nodes.len();
            let node = Node { id, out_links, weight: 1 };
            nodes.push(node);
        }

        println!("{:?}", nodes);
        println!("{:?}", links);
        (nodes, links)
    }

    /// This takes an example from https://github.com/LIHPC-Computational-Geometry/metis-rs/blob/master/examples/graph.rs
    /// Which itself is the example from https://raw.githubusercontent.com/KarypisLab/METIS/master/manual/manual.pdf
    /// chapter 5.5 - Figure 3
    #[rustfmt::skip]
    fn create_example() -> ([i32; 16], [i32; 44]) {
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
