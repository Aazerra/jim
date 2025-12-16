use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Object,      // {}
    Array,       // []
    String,      // "..."
    Number,      // 123, 12.34
    Boolean,     // true, false
    Null,        // null
    Key,         // object key
    Unknown,     // not yet parsed
    Error,       // parse error
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeKind::Object => write!(f, "Object"),
            NodeKind::Array => write!(f, "Array"),
            NodeKind::String => write!(f, "String"),
            NodeKind::Number => write!(f, "Number"),
            NodeKind::Boolean => write!(f, "Boolean"),
            NodeKind::Null => write!(f, "Null"),
            NodeKind::Key => write!(f, "Key"),
            NodeKind::Unknown => write!(f, "Unknown"),
            NodeKind::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseStatus {
    Unparsed,    // Not yet processed
    Parsing,     // Currently being parsed
    Parsed,      // Successfully parsed
    Invalid,     // Parse error
}

pub type NodeId = usize;

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub kind: NodeKind,
    pub start: usize,        // byte offset (inclusive)
    pub end: usize,          // byte offset (exclusive)
    pub depth: u8,           // nesting depth
    pub parent: Option<NodeId>,
    pub status: ParseStatus,
}

impl NodeInfo {
    pub fn new(
        kind: NodeKind,
        start: usize,
        end: usize,
        depth: u8,
        parent: Option<NodeId>,
    ) -> Self {
        Self {
            kind,
            start,
            end,
            depth,
            parent,
            status: ParseStatus::Parsed,
        }
    }

    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_container(&self) -> bool {
        matches!(self.kind, NodeKind::Object | NodeKind::Array)
    }

    pub fn is_value(&self) -> bool {
        matches!(
            self.kind,
            NodeKind::String | NodeKind::Number | NodeKind::Boolean | NodeKind::Null
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_contains() {
        let node = NodeInfo::new(NodeKind::Object, 10, 20, 0, None);
        assert!(node.contains(10));
        assert!(node.contains(15));
        assert!(node.contains(19));
        assert!(!node.contains(20));
        assert!(!node.contains(9));
    }

    #[test]
    fn test_node_is_container() {
        assert!(NodeInfo::new(NodeKind::Object, 0, 10, 0, None).is_container());
        assert!(NodeInfo::new(NodeKind::Array, 0, 10, 0, None).is_container());
        assert!(!NodeInfo::new(NodeKind::String, 0, 10, 0, None).is_container());
    }

    #[test]
    fn test_node_is_value() {
        assert!(NodeInfo::new(NodeKind::String, 0, 10, 0, None).is_value());
        assert!(NodeInfo::new(NodeKind::Number, 0, 10, 0, None).is_value());
        assert!(NodeInfo::new(NodeKind::Boolean, 0, 10, 0, None).is_value());
        assert!(NodeInfo::new(NodeKind::Null, 0, 10, 0, None).is_value());
        assert!(!NodeInfo::new(NodeKind::Object, 0, 10, 0, None).is_value());
    }
}
