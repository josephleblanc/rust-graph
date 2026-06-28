#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

impl Graph {
    pub fn new(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> Self {
        Self { nodes, edges }
    }

    pub fn sample() -> Self {
        Self {
            nodes: vec![
                GraphNode::new(0, "Core", [-1.2, 0.2, 0.0], 1.4),
                GraphNode::new(1, "UI", [0.0, 1.0, 0.5], 1.0),
                GraphNode::new(2, "Camera", [1.2, 0.3, -0.3], 1.0),
                GraphNode::new(3, "Layout", [0.3, -0.8, 0.6], 1.2),
                GraphNode::new(4, "Picking", [-1.0, -0.9, -0.6], 0.9),
                GraphNode::new(5, "Effects", [1.4, -0.8, 0.7], 0.9),
            ],
            edges: vec![
                GraphEdge::new(0, 1, 1.0),
                GraphEdge::new(0, 2, 1.0),
                GraphEdge::new(0, 3, 1.1),
                GraphEdge::new(1, 3, 0.8),
                GraphEdge::new(2, 5, 0.7),
                GraphEdge::new(3, 4, 0.8),
                GraphEdge::new(3, 5, 0.9),
            ],
        }
    }

    pub fn nodes(&self) -> &[GraphNode] {
        &self.nodes
    }

    pub fn node_by_id(&self, id: usize) -> Option<&GraphNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    pub id: usize,
    pub label: &'static str,
    pub position: [f32; 3],
    pub mass: f32,
}

impl GraphNode {
    pub const fn new(id: usize, label: &'static str, position: [f32; 3], mass: f32) -> Self {
        Self {
            id,
            label,
            position,
            mass,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphEdge {
    pub source: usize,
    pub target: usize,
    pub strength: f32,
}

impl GraphEdge {
    pub const fn new(source: usize, target: usize, strength: f32) -> Self {
        Self {
            source,
            target,
            strength,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForceLayoutSettings {
    pub repulsion: f32,
    pub attraction: f32,
    pub damping: f32,
    pub target_edge_length: f32,
}

impl Default for ForceLayoutSettings {
    fn default() -> Self {
        Self {
            repulsion: 1.6,
            attraction: 0.8,
            damping: 0.86,
            target_edge_length: 1.45,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_graph_edges_reference_existing_nodes() {
        let graph = Graph::sample();

        for edge in graph.edges() {
            assert!(graph.node_by_id(edge.source).is_some());
            assert!(graph.node_by_id(edge.target).is_some());
        }
    }

    #[test]
    fn node_lookup_uses_graph_ids_not_slice_indexes() {
        let graph = Graph::new(
            vec![GraphNode::new(42, "Node", [0.0, 0.0, 0.0], 1.0)],
            vec![],
        );

        assert_eq!(graph.node_by_id(42).map(|node| node.label), Some("Node"));
        assert!(graph.node_by_id(0).is_none());
    }
}
