#![allow(unused_mut)]
use basedrop::{Shared, Collector};
pub enum PortKind {
    Audio, 
    Event,
}

pub struct Port {
    pub name:&'static str,
    pub kind:PortKind,
}

pub struct Node {
    pub name: Shared<String>,
    pub inputs: Shared<Vec<Port>>,
    pub outputs: Shared<Vec<Port>>,
}

pub struct NodeBuilder {
    inputs:Vec<Port>,
    outputs:Vec<Port>,
}

pub fn node() -> NodeBuilder {
    NodeBuilder { inputs: vec![], outputs: vec![] }
}

impl NodeBuilder {
    pub fn port (mut self, is_input:bool, kind:PortKind, name:&'static str) -> Self {
        let v = if is_input { &mut self.inputs } else { &mut self.outputs }; 
        debug_assert!(v.iter().find(|port| port.name == name).is_none()); 
        v.push(Port { name, kind });
        self
    }
    pub fn audio_input(mut self, name:&'static str) -> Self {
        self.port(true, PortKind::Audio, name)
    }
    pub fn event_input(mut self, name:&'static str) -> Self {
        self.port(true, PortKind::Event, name)
    }
    pub fn audio_output(mut self, name:&'static str) -> Self {
        self.port(false, PortKind::Audio, name)
    }
    pub fn event_output(mut self, name:&'static str) -> Self {
        self.port(false, PortKind::Event, name)
    }
    pub fn build (self, gc:&Collector, name: &str) -> Node {
        Node {
            name: Shared::new(&gc.handle(), name.to_owned()),
            inputs: Shared::new(&gc.handle(), self.inputs), 
            outputs: Shared::new(&gc.handle(), self.outputs),
        }
    }
}
