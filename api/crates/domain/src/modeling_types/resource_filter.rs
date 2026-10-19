use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceFilterExpr {
    All(Vec<ResourceFilterExpr>),
    Any(Vec<ResourceFilterExpr>),
    Field {
        field: String,
        operator: ResourceFilterOperator,
        value: Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceFilterOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    Includes,
    NotIncludes,
    In,
}

impl ResourceFilterExpr {
    pub fn all(items: Vec<Self>) -> Self {
        let mut flattened = Vec::new();
        for item in items {
            match item {
                Self::All(items) => flattened.extend(items),
                item => flattened.push(item),
            }
        }
        if flattened.len() == 1 {
            flattened.remove(0)
        } else {
            Self::All(flattened)
        }
    }

    pub fn any(items: Vec<Self>) -> Self {
        let mut flattened = Vec::new();
        for item in items {
            match item {
                Self::Any(items) => flattened.extend(items),
                item => flattened.push(item),
            }
        }
        if flattened.len() == 1 {
            flattened.remove(0)
        } else {
            Self::Any(flattened)
        }
    }
}

impl ResourceFilterOperator {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "$eq" => Some(Self::Eq),
            "$ne" => Some(Self::Ne),
            "$gt" => Some(Self::Gt),
            "$gte" => Some(Self::Gte),
            "$lt" => Some(Self::Lt),
            "$lte" => Some(Self::Lte),
            "$includes" => Some(Self::Includes),
            "$notIncludes" => Some(Self::NotIncludes),
            "$in" => Some(Self::In),
            _ => None,
        }
    }
}
