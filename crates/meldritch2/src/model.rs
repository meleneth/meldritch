use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Endpoint {
    pub module: String,
    pub port: String,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub id: String,
    pub kind: String,
    pub values: BTreeMap<String, f64>,
}

#[derive(Clone, Debug)]
pub struct Connection {
    pub from: Endpoint,
    pub to: Endpoint,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub modules: Vec<Module>,
    pub connections: Vec<Connection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ParameterId {
    pub module: String,
    pub name: String,
}

impl ParameterId {
    pub fn new(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            name: name.into(),
        }
    }
}
