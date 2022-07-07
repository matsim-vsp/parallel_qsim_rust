use metis::Graph;

#[derive(Debug)]
struct Node {
    id: usize,
    out_links: Vec<usize>,
}

#[derive(Debug)]
struct Link {
    id: usize,
    count: i32,
    to: usize,
}

fn convert(nodes: Vec<Node>, links: Vec<Link>) -> Vec<i32> {
    let mut xadj: Vec<i32> = Vec::from([0]);
    let mut adjncy: Vec<i32> = Vec::new();
    let mut adjwgt: Vec<i32> = Vec::new();

    for node in nodes {
        // do the xadj  pointers.
        let number_of_out_links = node.out_links.len() as i32;
        let next_adjacency_index = xadj.last().unwrap() + number_of_out_links;
        xadj.push(next_adjacency_index as i32);

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

    // ncon specifies number of vertice weights. Our vertices are unweighted
    // nparts is the number of parts. We'll start with 2
    Graph::new(1, 2, &mut xadj.as_mut_slice(), &mut adjncy.as_mut_slice())
        .part_kway(&mut result)
        .unwrap();

    result
}

#[cfg(test)]
mod tests {
    use metis::Graph;

    use crate::experiments::metis_test::{convert, Link, Node};

    #[test]
    fn test_convert() {
        let (nodes, links) = create_network();

        let result = convert(nodes, links);

        let mut expected = vec![0x00; 15];
        let mut crs = create_example();
        Graph::new(1, 2, &mut crs.0, &mut crs.1)
            .part_kway(&mut expected)
            .unwrap();

        assert_eq!(expected, result)
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
            let node = Node { id, out_links };
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
