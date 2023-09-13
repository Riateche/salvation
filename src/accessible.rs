use std::collections::{hash_map::Entry, HashMap, HashSet};

use accesskit::{NodeBuilder, NodeClassSet, NodeId, Role, Tree, TreeUpdate};

use crate::widgets::RawWidgetId;

pub struct AccessibleNodes {
    pub nodes: HashMap<NodeId, NodeBuilder>,
    pub direct_children: HashMap<NodeId, HashSet<NodeId>>,
    pub direct_parents: HashMap<NodeId, NodeId>,

    pub pending_updates: HashSet<NodeId>,
    pub classes: NodeClassSet,
    pub root: NodeId,
    pub focus: NodeId,
}

impl AccessibleNodes {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let root = RawWidgetId::new().0.into();
        let mut this = Self {
            nodes: Default::default(),
            direct_children: Default::default(),
            direct_parents: Default::default(),
            pending_updates: Default::default(),
            classes: Default::default(),
            root,
            focus: root,
        };
        this.clear();
        this
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.pending_updates.clear();

        let root_node = NodeBuilder::new(Role::Group);
        self.update(self.root, Some(root_node));
    }

    pub fn mount(&mut self, parent: Option<NodeId>, child: NodeId) {
        // TODO: stricter checks and warnings
        let parent = parent.unwrap_or(self.root);
        self.direct_parents.insert(child, parent);
        self.direct_children
            .entry(parent)
            .or_default()
            .insert(child);
        self.mark_parent_as_pending(parent);
    }

    pub fn unmount(&mut self, parent: Option<NodeId>, child: NodeId) {
        // TODO: stricter checks and warnings
        let parent = parent.unwrap_or(self.root);
        self.direct_parents.remove(&parent);
        if let Entry::Occupied(mut entry) = self.direct_children.entry(parent) {
            entry.get_mut().remove(&child);
            if entry.get_mut().is_empty() {
                entry.remove();
            }
        }
        self.mark_parent_as_pending(parent);
    }

    fn mark_parent_as_pending(&mut self, mut parent: NodeId) {
        loop {
            if self.nodes.contains_key(&parent) {
                self.pending_updates.insert(parent);
                break;
            } else if parent == self.root {
                println!("warn: node not found for root");
                break;
            } else if let Some(next) = self.direct_parents.get(&parent) {
                parent = *next;
            } else {
                println!("warn: parent not found");
                break;
            }
        }
    }

    pub fn update(&mut self, id: NodeId, node: Option<NodeBuilder>) {
        let added_or_removed;
        if let Some(node) = node {
            let r = self.nodes.insert(id, node);
            added_or_removed = r.is_none();
        } else {
            let r = self.nodes.remove(&id);
            added_or_removed = r.is_some();
        }
        self.pending_updates.insert(id);
        if added_or_removed && id != self.root {
            if let Some(parent) = self.direct_parents.get(&id) {
                self.mark_parent_as_pending(*parent);
            } else {
                println!("warn: parent not found");
            }
        }
    }

    pub fn set_focus(&mut self, focus: Option<NodeId>) {
        // TODO: what if this node or root are not focused?
        self.focus = focus.unwrap_or(self.root);
    }

    pub fn take_update(&mut self) -> TreeUpdate {
        let mut nodes = Vec::new();
        for id in self.pending_updates.drain() {
            if let Some(node) = self.nodes.get(&id) {
                let mut children = Vec::new();
                find_children(id, &self.direct_children, &self.nodes, &mut children);
                let mut node = node.clone();
                node.set_children(children);
                nodes.push((id, node.build(&mut self.classes)));
            }
        }
        TreeUpdate {
            nodes,
            tree: Some(Tree { root: self.root }),
            focus: self.focus,
        }
    }
}

fn find_children(
    parent: NodeId,
    direct_children: &HashMap<NodeId, HashSet<NodeId>>,
    nodes: &HashMap<NodeId, NodeBuilder>,
    out: &mut Vec<NodeId>,
) {
    if let Some(children) = direct_children.get(&parent) {
        for child in children {
            if nodes.contains_key(child) {
                out.push(*child);
            } else {
                find_children(*child, direct_children, nodes, out);
            }
        }
    }
}
