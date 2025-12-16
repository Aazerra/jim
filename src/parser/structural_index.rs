use super::node::{NodeId, NodeInfo, NodeKind};
use super::token::{Token, TokenKind};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StructuralIndex {
    nodes: Vec<NodeInfo>,
    // Quick lookup: byte offset -> node index
    offset_map: HashMap<usize, NodeId>,
}

impl StructuralIndex {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            offset_map: HashMap::new(),
        }
    }

    /// Build index from token stream
    pub fn from_tokens(tokens: &[Token]) -> Self {
        let mut index = Self::new();
        let mut stack: Vec<(NodeId, NodeKind, usize, u32)> = Vec::new(); // (id, kind, start, depth)
        
        for token in tokens {
            match token.kind {
                TokenKind::BraceOpen => {
                    let node_id = index.nodes.len();
                    let parent = stack.last().map(|(id, _, _, _)| *id);
                    
                    // Create placeholder node (will update end when we see closing brace)
                    let node = NodeInfo::new(
                        NodeKind::Object,
                        token.start,
                        token.end, // Temporary, will update
                        token.depth as u8,
                        parent,
                    );
                    
                    index.nodes.push(node);
                    index.offset_map.insert(token.start, node_id);
                    stack.push((node_id, NodeKind::Object, token.start, token.depth));
                }
                
                TokenKind::BracketOpen => {
                    let node_id = index.nodes.len();
                    let parent = stack.last().map(|(id, _, _, _)| *id);
                    
                    let node = NodeInfo::new(
                        NodeKind::Array,
                        token.start,
                        token.end, // Temporary
                        token.depth as u8,
                        parent,
                    );
                    
                    index.nodes.push(node);
                    index.offset_map.insert(token.start, node_id);
                    stack.push((node_id, NodeKind::Array, token.start, token.depth));
                }
                
                TokenKind::BraceClose | TokenKind::BracketClose => {
                    if let Some((node_id, _, _, _)) = stack.pop() {
                        // Update the end position of the container
                        if let Some(node) = index.nodes.get_mut(node_id) {
                            node.end = token.end;
                        }
                    }
                }
                
                TokenKind::String => {
                    let node_id = index.nodes.len();
                    let parent = stack.last().map(|(id, _, _, _)| *id);
                    
                    let node = NodeInfo::new(
                        NodeKind::String,
                        token.start,
                        token.end,
                        token.depth as u8,
                        parent,
                    );
                    
                    index.nodes.push(node);
                    index.offset_map.insert(token.start, node_id);
                }
                
                TokenKind::Number => {
                    let node_id = index.nodes.len();
                    let parent = stack.last().map(|(id, _, _, _)| *id);
                    
                    let node = NodeInfo::new(
                        NodeKind::Number,
                        token.start,
                        token.end,
                        token.depth as u8,
                        parent,
                    );
                    
                    index.nodes.push(node);
                    index.offset_map.insert(token.start, node_id);
                }
                
                TokenKind::True | TokenKind::False => {
                    let node_id = index.nodes.len();
                    let parent = stack.last().map(|(id, _, _, _)| *id);
                    
                    let node = NodeInfo::new(
                        NodeKind::Boolean,
                        token.start,
                        token.end,
                        token.depth as u8,
                        parent,
                    );
                    
                    index.nodes.push(node);
                    index.offset_map.insert(token.start, node_id);
                }
                
                TokenKind::Null => {
                    let node_id = index.nodes.len();
                    let parent = stack.last().map(|(id, _, _, _)| *id);
                    
                    let node = NodeInfo::new(
                        NodeKind::Null,
                        token.start,
                        token.end,
                        token.depth as u8,
                        parent,
                    );
                    
                    index.nodes.push(node);
                    index.offset_map.insert(token.start, node_id);
                }
                
                _ => {} // Ignore whitespace, colons, commas
            }
        }
        
        index
    }

    /// Find node at byte offset using binary search
    pub fn node_at(&self, offset: usize) -> Option<&NodeInfo> {
        // Binary search for node containing offset
        self.nodes
            .binary_search_by(|node| {
                if offset < node.start {
                    std::cmp::Ordering::Greater
                } else if offset >= node.end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
            .and_then(|idx| self.nodes.get(idx))
    }

    /// Get node by ID
    pub fn get(&self, id: NodeId) -> Option<&NodeInfo> {
        self.nodes.get(id)
    }

    /// Find next sibling at same depth
    pub fn next_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let node = self.nodes.get(node_id)?;
        let depth = node.depth;
        let parent = node.parent;
        
        // Find next node with same depth and parent
        for (idx, n) in self.nodes.iter().enumerate().skip(node_id + 1) {
            if n.depth == depth && n.parent == parent {
                return Some(idx);
            }
            // Stop if we've left the parent container
            if n.depth < depth {
                break;
            }
        }
        
        None
    }

    /// Find previous sibling at same depth
    pub fn prev_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let node = self.nodes.get(node_id)?;
        let depth = node.depth;
        let parent = node.parent;
        
        // Search backwards
        for idx in (0..node_id).rev() {
            let n = &self.nodes[idx];
            if n.depth == depth && n.parent == parent {
                return Some(idx);
            }
            // Stop if we've left the parent container
            if n.depth < depth {
                break;
            }
        }
        
        None
    }

    /// Get parent node
    pub fn parent(&self, node_id: NodeId) -> Option<NodeId> {
        self.nodes.get(node_id)?.parent
    }
    
    /// Get first child of a container node
    pub fn first_child(&self, node_id: NodeId) -> Option<NodeId> {
        let node = self.nodes.get(node_id)?;
        let child_depth = node.depth + 1;
        
        // Find first node with depth = parent_depth + 1 and parent = node_id
        for (idx, n) in self.nodes.iter().enumerate().skip(node_id + 1) {
            if n.parent == Some(node_id) && n.depth == child_depth {
                return Some(idx);
            }
            // Stop if we've left the container
            if n.depth <= node.depth {
                break;
            }
        }
        
        None
    }

    /// Get all children of a container node
    pub fn children(&self, node_id: NodeId) -> Vec<NodeId> {
        let node = match self.nodes.get(node_id) {
            Some(n) if n.is_container() => n,
            _ => return Vec::new(),
        };
        
        let mut children = Vec::new();
        let depth = node.depth;
        
        for (idx, n) in self.nodes.iter().enumerate().skip(node_id + 1) {
            // Stop when we exit this container
            if n.start >= node.end {
                break;
            }
            // Only direct children (depth = parent depth + 1)
            if n.depth == depth + 1 {
                children.push(idx);
            }
        }
        
        children
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn nodes(&self) -> &[NodeInfo] {
        &self.nodes
    }
    
    /// Find the next key node after the given offset
    /// Keys are String nodes that are direct children of Object nodes
    pub fn next_key(&self, from_offset: usize) -> Option<NodeId> {
        // Find all String nodes that are object keys (odd-indexed children of objects)
        for (idx, node) in self.nodes.iter().enumerate() {
            if node.start <= from_offset {
                continue;
            }
            
            // Check if this is a key (String node in an object)
            if node.kind == NodeKind::String {
                if let Some(parent_id) = node.parent {
                    if let Some(parent) = self.nodes.get(parent_id) {
                        if parent.kind == NodeKind::Object {
                            // In JSON objects, keys are strings at even positions among siblings
                            // Check if this is at a key position
                            let siblings: Vec<_> = self.children(parent_id);
                            if let Some(pos) = siblings.iter().position(|&id| id == idx) {
                                if pos % 2 == 0 {
                                    return Some(idx);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
    
    /// Find the previous key node before the given offset
    pub fn prev_key(&self, from_offset: usize) -> Option<NodeId> {
        let mut result = None;
        
        for (idx, node) in self.nodes.iter().enumerate() {
            if node.start >= from_offset {
                break;
            }
            
            // Check if this is a key
            if node.kind == NodeKind::String {
                if let Some(parent_id) = node.parent {
                    if let Some(parent) = self.nodes.get(parent_id) {
                        if parent.kind == NodeKind::Object {
                            let siblings: Vec<_> = self.children(parent_id);
                            if let Some(pos) = siblings.iter().position(|&id| id == idx) {
                                if pos % 2 == 0 {
                                    result = Some(idx);
                                }
                            }
                        }
                    }
                }
            }
        }
        result
    }
    
    /// Find the next value node after the given offset
    /// Values can be any node type that's not a key
    pub fn next_value(&self, from_offset: usize) -> Option<NodeId> {
        for (idx, node) in self.nodes.iter().enumerate() {
            if node.start <= from_offset {
                continue;
            }
            
            // Check if this is a value node
            if self.is_value_node(idx) {
                return Some(idx);
            }
        }
        None
    }
    
    /// Find the previous value node before the given offset
    pub fn prev_value(&self, from_offset: usize) -> Option<NodeId> {
        let mut result = None;
        
        for (idx, node) in self.nodes.iter().enumerate() {
            if node.start >= from_offset {
                break;
            }
            
            if self.is_value_node(idx) {
                result = Some(idx);
            }
        }
        result
    }
    
    /// Check if a node is a value (not a key in an object)
    fn is_value_node(&self, node_id: NodeId) -> bool {
        let node = match self.nodes.get(node_id) {
            Some(n) => n,
            None => return false,
        };
        
        // If parent is an object, check if this is an odd-indexed child (value position)
        if let Some(parent_id) = node.parent {
            if let Some(parent) = self.nodes.get(parent_id) {
                if parent.kind == NodeKind::Object {
                    let siblings: Vec<_> = self.children(parent_id);
                    if let Some(pos) = siblings.iter().position(|&id| id == node_id) {
                        // Odd positions are values in objects
                        return pos % 2 == 1;
                    }
                } else if parent.kind == NodeKind::Array {
                    // All children of arrays are values
                    return true;
                }
            }
        }
        
        // Top-level nodes are values
        node.parent.is_none()
    }
}

impl Default for StructuralIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Tokenizer;

    #[test]
    fn test_index_from_simple_object() {
        let json = r#"{"key": "value"}"#;
        let mut tokenizer = Tokenizer::new(json.to_string());
        let tokens = tokenizer.tokenize_all();
        
        let index = StructuralIndex::from_tokens(&tokens);
        assert!(!index.is_empty());
        
        // Should have object, key string, and value string
        assert!(index.len() >= 3);
        
        // First node should be object
        let first = index.get(0).unwrap();
        assert_eq!(first.kind, NodeKind::Object);
    }

    #[test]
    fn test_node_at_offset() {
        let json = r#"{"key": "value"}"#;
        let mut tokenizer = Tokenizer::new(json.to_string());
        let tokens = tokenizer.tokenize_all();
        let index = StructuralIndex::from_tokens(&tokens);
        
        // Offset 0 should be in the object
        let node = index.node_at(0);
        assert!(node.is_some());
        assert_eq!(node.unwrap().kind, NodeKind::Object);
    }

    #[test]
    fn test_sibling_navigation() {
        let json = r#"[1, 2, 3]"#;
        let mut tokenizer = Tokenizer::new(json.to_string());
        let tokens = tokenizer.tokenize_all();
        let index = StructuralIndex::from_tokens(&tokens);
        
        // Find first number (1)
        let first_num_id = index.nodes()
            .iter()
            .position(|n| n.kind == NodeKind::Number)
            .unwrap();
        
        // Should have a next sibling
        let next = index.next_sibling(first_num_id);
        assert!(next.is_some());
        
        // Next sibling should also be a number
        let next_node = index.get(next.unwrap()).unwrap();
        assert_eq!(next_node.kind, NodeKind::Number);
    }
}
