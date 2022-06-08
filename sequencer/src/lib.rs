// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use q_filter_tree::{error::InvalidNodePath, id::NodePathTyped, Tree};

pub struct Sequencer {
    tree: Tree<String, String>,
}
impl Sequencer {
    pub fn new() -> Self {
        Self { tree: Tree::new() }
    }
    pub fn add_node(&mut self, parent_path_str: String) -> Result<String, Error> {
        let parent_path = parse_path_str(parent_path_str)?;
        let mut parent_ref = parent_path.try_ref(&mut self.tree)?;
        let mut child_nodes = parent_ref
            .child_nodes()
            .ok_or_else(|| format!("Node {parent_path} does not have child_nodes"))?;
        let node_id = child_nodes.add_child_default();
        let node_id_str = serde_json::to_string(&NodePathTyped::from(node_id))?;
        Ok(node_id_str)
    }
    pub fn set_node_file(&mut self, node_path_str: String, filename: String) -> Result<(), Error> {
        let node_path = parse_path_str(node_path_str)?;
        let mut node_ref = node_path.try_ref(&mut self.tree)?;
        todo!()
    }
}

fn parse_path_str(path_str: String) -> Result<NodePathTyped, String> {
    path_str
        .parse()
        .map_err(|(parse_int_err, elem_str)| format!("{parse_int_err}: \"{elem_str}\""))
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Sequencer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tree = serde_json::to_string_pretty(&self.tree)
            .unwrap_or_else(|err| format!("<<error: {err}>>"));
        write!(f, "Sequencer {tree}")
    }
}

shared::wrapper_enum! {
    #[derive(Debug)]
    pub enum Error {
        Message(String),
        Serde(serde_json::Error),
        InvalidNodePath(InvalidNodePath),
    }
}
