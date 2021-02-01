use std::collections::HashMap;

use crate::{
    audio_buffer::{
        channel_description, AudioBuffer, ChannelConfiguration, RefBuffer, RefBufferMut,
    },
    audio_stream::PlaybackContext,
};
use arrayvec::{ArrayString, CapacityError};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use thiserror::Error as ThisError;

/// An error that may arise from graph operations
/// [todo] add more detailed error information
#[derive(ThisError, Debug)]
pub enum Error {
    #[error("source and destination port configurations do not match")]
    PortConfigError,
    #[error("port does not exist")]
    PortDoesNotExist,
    #[error("node does not exist")]
    NodeDoesNotExist,
    #[error("node already exists with that name")]
    NodeAlreadyExists,
    #[error("port already exists with that name")]
    PortAlreadyExists,
    #[error("string length error")]
    StringCapacityError,
    #[error("connection is invalid, or would create cycle")]
    InvalidConnection,
    #[error("connection already exists")]
    ConnectionAlreadyExists,
    #[error("connection does not exist")]
    ConnectionDoesNotExist,
}

/// The direction of a port
pub enum Direction {
    /// An input, receives data
    Input,
    /// An output, sources data
    Output,
}

#[derive(Copy, Clone, Debug)]
struct Port {
    channel_config: ChannelConfiguration,
    name: ArrayString<[u8; 32]>,
}

struct Node {
    // Nodes have unique names, max 32 characters long.
    name: ArrayString<[u8; 32]>,
    // Nodes have a list of input ports
    inputs: Vec<(u64, Port)>,
    // Nodes hav a list of output ports
    outputs: Vec<(u64, Port)>,
    // A node's dependencies is a list of incoming edges, mapping source node, source
    // port, to this node's destination port.
    dependencies: Vec<(u64, u64, u64)>,
    // A node's dependents is a list of outgoing edges, mapping destination node, destination
    // port, to this node's source port.
    dependents: Vec<(u64, u64, u64)>,
}

/// The [Graph] structure is used to represent connections in the engine, or mappings
/// of input to output.
#[derive(Default)]
pub struct Graph {
    nodes: HashMap<u64, Node>,
}

/// A cheap to copy reference/handle to a node in the graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeHandle(u64);

/// Nodes have ports corresponding to their inputs and outputs, the port handle is a cheap
/// to copy reference/handle to a port that belongs to a node. Ports are unique relative
/// to their direction and parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortHandle(u64);

impl Direction {
    /// Returns true if the direction is an input
    #[inline]
    pub fn is_input(&self) -> bool {
        match self {
            Direction::Input => true,
            Direction::Output => false,
        }
    }
    /// Returns true if the direction is an output
    #[inline]
    pub fn is_output(&self) -> bool {
        !self.is_input()
    }
}

impl<'a> From<CapacityError<&'a str>> for Error {
    fn from(_: CapacityError<&'a str>) -> Self {
        Error::StringCapacityError
    }
}

// helper to hash strings into identifiers.
fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

impl Graph {
    /// Add a node to the graph with a fixed name. Returns an error if a node by that name already exists.
    pub fn add_node(&mut self, name: &str) -> Result<NodeHandle, Error> {
        let id: u64 = hash_str(name);
        if self.nodes.contains_key(&id) {
            return Err(Error::NodeAlreadyExists);
        }

        let node = Node {
            name: ArrayString::from(name)?,
            inputs: vec![],
            outputs: vec![],
            dependencies: vec![],
            dependents: vec![],
        };

        let _ = self.nodes.insert(id, node);
        Ok(NodeHandle(id))
    }

    /// Add a port to a node in the graph with a name. Returns an error if a port by that name already exists.
    pub fn add_port(
        &mut self,
        direction: Direction,
        node: NodeHandle,
        port_name: &str,
    ) -> Result<PortHandle, Error> {
        let port_id = hash_str(port_name);
        let node = self.nodes.get_mut(&node.0).ok_or(Error::NodeDoesNotExist)?;
        let ports = match direction {
            Direction::Input => &mut node.inputs,
            Direction::Output => &mut node.outputs,
        };
        if ports.iter().find(|(id, _)| *id == port_id).is_some() {
            return Err(Error::PortAlreadyExists);
        }
        let port = Port {
            name: ArrayString::from(port_name)?,
            channel_config: channel_description::mono(),
        };
        ports.push((port_id, port));
        Ok(PortHandle(port_id))
    }

    fn get_node(&self, id: NodeHandle) -> Result<&'_ Node, Error> {
        self.nodes.get(&id.0).ok_or(Error::NodeDoesNotExist)
    }

    fn get_node_mut(&mut self, id: NodeHandle) -> Result<&'_ mut Node, Error> {
        self.nodes.get_mut(&id.0).ok_or(Error::NodeDoesNotExist)
    }

    /// Configure a port. Returns an error if the port or node does not exist. All connections to the port will be invalidated.
    pub fn configure_port(
        &mut self,
        direction: Direction,
        node: NodeHandle,
        port: PortHandle,
        config: ChannelConfiguration,
    ) -> Result<(), Error> {
        // first reconfigure the port
        let this_node = self.get_node_mut(node)?;
        let ports = match direction {
            Direction::Input => &mut this_node.inputs,
            Direction::Output => &mut this_node.outputs,
        };
        let (_, this_port) = ports
            .iter_mut()
            .find(|(id, _)| *id == port.0)
            .ok_or(Error::PortDoesNotExist)?;
        this_port.channel_config = config;
        // next, remove connections
        // todo: remove allocations to please the borrow checker
        match direction {
            Direction::Input => {
                let deps = this_node.dependencies.clone();
                for (src, src_port, _) in deps.into_iter().filter(|(_, _, p)| *p == port.0) {
                    // ignore errors here. We don't actually care if this fails, since it means we
                    // did the correct thing (remove an edge to nodes/ports that don't exist, or an edge
                    // that doesn't exist),
                    //
                    // We could use an expect(), but then if our assumptions turn out to be wrong the program
                    // will crash. Proper error handling would be to collect the errors into a buffer or to
                    // log warnings.
                    //
                    // todo: log warning
                    let _ = self.disconnect((NodeHandle(src), PortHandle(src_port)), (node, port));
                }
            }
            Direction::Output => {
                let deps = this_node.dependents.clone();
                for (dst, dst_port, _) in deps.into_iter().filter(|(_, _, p)| *p == port.0) {
                    // ignore errors here. Same reasons as above.
                    //
                    // todo: log warning
                    let _ = self.disconnect((node, port), (NodeHandle(dst), PortHandle(dst_port)));
                }
            }
        }
        Ok(())
    }
    /// Delete a node in the graph. Returns an error if the node does not exist, and removes all edgs that reach
    /// this node.
    pub fn del_node(&mut self, node: NodeHandle) -> Result<(), Error> {
        let this_node = self.nodes.remove(&node.0).ok_or(Error::NodeDoesNotExist)?;
        let (dependencies, dependents) = (this_node.dependencies, this_node.dependents);
        for (src, _, _) in dependencies {
            // todo: identify if this condition is possible. It probably isn't, however we don't want to return early from
            //       this function and leave the graph in invalid state.
            // todo: log warning
            if let Ok(src) = self.get_node_mut(NodeHandle(src)) {
                let dependents = src
                    .dependents
                    .iter()
                    .filter_map(|d| if d.0 == node.0 { None } else { Some(*d) })
                    .collect();
                src.dependents = dependents;
            }
        }
        for (dst, _, _) in dependents {
            // todo: log warning
            if let Ok(dst) = self.get_node_mut(NodeHandle(dst)) {
                let dependencies = dst
                    .dependencies
                    .iter()
                    .filter_map(|d| if d.0 == node.0 { None } else { Some(*d) })
                    .collect();
                dst.dependencies = dependencies;
            }
        }
        Ok(())
    }
    /// Delete a port from a node in the graph. Returns an error if the node or port does not exist.
    pub fn del_port(
        &mut self,
        direction: Direction,
        node: NodeHandle,
        port: PortHandle,
    ) -> Result<(), Error> {
        let this_node = self.get_node_mut(node)?;
        // first, remove the port
        {
            let ports = match direction {
                Direction::Input => &mut this_node.inputs,
                Direction::Output => &mut this_node.outputs,
            };
            let port_index = ports
                .iter()
                .position(|(id, _)| *id == port.0)
                .ok_or(Error::PortDoesNotExist)?;
            ports.remove(port_index);
        }
        // remove any edges that reach it
        {
            // todo: remove the clone to please the borrow checker. We guarantee not to be operating
            //       on the same data when performing these operations.
            let deps = match direction {
                Direction::Input => this_node.dependencies.clone(),
                Direction::Output => this_node.dependents.clone(),
            };
            for (next_node, _, _) in deps.into_iter().filter(|(_, _, p)| *p == port.0) {
                // todo: identify if this case can occur. We skip over errors and remove what we can
                //       rather than exiting early and leaving the graph in invalid state.
                //
                // todo: log warning
                if let Ok(next_node) = self.get_node_mut(NodeHandle(next_node)) {
                    let next_deps = match direction {
                        Direction::Input => &mut next_node.dependents,
                        Direction::Output => &mut next_node.dependencies,
                    };
                    if let Some(dep_index) = next_deps
                        .iter()
                        .position(|(node_id, port_id, _)| *node_id == node.0 && *port_id == port.0)
                    {
                        next_deps.remove(dep_index);
                    }
                }
            }
        }
        Ok(())
    }
    /// Connect a source node and port to a destination node and port. Returns an error if any of the nodes
    /// or ports do not exist, if the configurations of the ports do not match, or if the connection would create
    /// a cycle in the graph.
    pub fn connect(
        &mut self,
        src: (NodeHandle, PortHandle),
        dst: (NodeHandle, PortHandle),
    ) -> Result<(), Error> {
        self.connection_check(src, dst)?;
        let (src, src_port) = src;
        let (dst, dst_port) = dst;
        {
            // check that our nodes and ports exist and their configurations agree.
            let src_node = self.get_node(src)?;
            let dst_node = self.get_node(dst)?;
            let (_, src_port) = src_node
                .outputs
                .iter()
                .find(|(id, _)| *id == src_port.0)
                .ok_or(Error::PortDoesNotExist)?;
            let (_, dst_port) = dst_node
                .inputs
                .iter()
                .find(|(id, _)| *id == dst_port.0)
                .ok_or(Error::PortDoesNotExist)?;
            if src_port.channel_config != dst_port.channel_config {
                return Err(Error::PortConfigError);
            }
        }
        // check if the connection would create a cycle
        self.cycle_check(dst.0, src.0)?;
        let dependency = (src.0, src_port.0, dst_port.0);
        // now we can add the connection
        let dst_node = self.get_node_mut(dst)?;
        // check if the connection already exists
        let d = dst_node.dependencies.iter().find(|e| **e == dependency);
        if d.is_some() {
            return Err(Error::ConnectionAlreadyExists);
        }
        dst_node.dependencies.push(dependency);
        let dependent = (dst.0, dst_port.0, src_port.0);
        let src_node = self.get_node_mut(src)?;
        // todo: this checking may be redundant.
        let d = src_node.dependents.iter().find(|e| **e == dependency);
        if d.is_some() {
            return Err(Error::ConnectionAlreadyExists);
        }
        src_node.dependents.push(dependent);
        Ok(())
    }
    // check if a connection would create a cycle.
    fn cycle_check(&self, node_id: u64, check_id: u64) -> Result<(), Error> {
        if node_id == check_id {
            return Err(Error::InvalidConnection);
        } else if let Ok(next_node) = self.get_node(NodeHandle(node_id)) {
            // Errors can be safely ignored here.
            for (next_node, _, _) in &next_node.dependencies {
                self.cycle_check(*next_node, check_id)?;
            }
        }
        Ok(())
    }
    // check if a connection is valid by looking for its source/destination
    // nodes and ports.
    fn connection_check(
        &self,
        src: (NodeHandle, PortHandle),
        dst: (NodeHandle, PortHandle),
    ) -> Result<(), Error> {
        let (src, src_port) = src;
        let (dst, dst_port) = dst;
        // check that our nodes and ports exist and their configurations agree.
        let src_node = self.get_node(src)?;
        let dst_node = self.get_node(dst)?;
        let (_, src_port) = src_node
            .outputs
            .iter()
            .find(|(id, _)| *id == src_port.0)
            .ok_or(Error::PortDoesNotExist)?;
        let (_, dst_port) = dst_node
            .inputs
            .iter()
            .find(|(id, _)| *id == dst_port.0)
            .ok_or(Error::PortDoesNotExist)?;
        if src_port.channel_config != dst_port.channel_config {
            return Err(Error::PortConfigError);
        }
        Ok(())
    }
    /// Disconnect a source node and port to a destination node and port. Returns an error if any of the nodes
    /// or ports do not exist or if the connection did not exist.
    pub fn disconnect(
        &mut self,
        src: (NodeHandle, PortHandle),
        dst: (NodeHandle, PortHandle),
    ) -> Result<(), Error> {
        self.connection_check(src, dst)?;
        let (src, src_port) = src;
        let (dst, dst_port) = dst;
        let (src_node, dst_node) = (self.get_node(src)?, self.get_node(dst)?);
        let dependency = (src.0, src_port.0, dst_port.0);
        let dependent = (dst.0, dst_port.0, src_port.0);
        let dependency_idx = dst_node
            .dependencies
            .iter()
            .position(|d| *d == dependency)
            .ok_or(Error::ConnectionDoesNotExist)?;
        let dependent_idx = src_node
            .dependents
            .iter()
            .position(|d| *d == dependent)
            .ok_or(Error::ConnectionDoesNotExist)?;
        let _ = self.get_node_mut(src)?.dependents.remove(dependent_idx);
        let _ = self.get_node_mut(dst)?.dependencies.remove(dependency_idx);
        Ok(())
    }
    /// Get the name of a node
    pub fn node_name(&self, node: NodeHandle) -> Result<&'_ str, Error> {
        Ok(&self.get_node(node)?.name)
    }
    /// Get the name of a port
    pub fn port_name(
        &self,
        direction: Direction,
        node: NodeHandle,
        port: PortHandle,
    ) -> Result<&'_ str, Error> {
        let node = self.get_node(node)?;
        let ports = match direction {
            Direction::Input => &node.inputs,
            Direction::Output => &node.outputs,
        };
        ports
            .iter()
            .find_map(|(id, p)| {
                if *id == port.0 {
                    Some(p.name.as_str())
                } else {
                    None
                }
            })
            .ok_or(Error::PortDoesNotExist)
    }

    /// get the inputs of a node
    pub fn list_inputs(
        &self,
        node: NodeHandle,
    ) -> Result<impl Iterator<Item = PortHandle> + '_, Error> {
        Ok(self
            .get_node(node)?
            .inputs
            .iter()
            .map(|(id, _)| PortHandle(*id)))
    }

    /// get the outputs of a node
    pub fn list_outputs(
        &self,
        node: NodeHandle,
    ) -> Result<impl Iterator<Item = PortHandle> + '_, Error> {
        Ok(self
            .get_node(node)?
            .outputs
            .iter()
            .map(|(id, _)| PortHandle(*id)))
    }

    /// The schedule is the data structure containing all the information required to render
    /// the graph, including a topological ordering, buffer allocations, and latency compensation.
    ///
    /// This method rebuilds the schedule, which can be passed to the audio thread
    ///
    /// The `root` element is the final output of the graph, normally the master mix bus. The graph
    /// doesn't care about what that element is, so it's up to the caller to notate which one they care about.
    ///
    /// `get_node_process` and `get_node_delay` are functions that the caller must implement to inform the
    /// graph about how to process buffers once they've been assigned in the schedule, and how long it takes
    /// the node to perform the process in order to compensate for that delay.
    pub fn reschedule(
        &self,
        // The root element of the graph to pull from (final output, usually the master mixbus)
        root: NodeHandle,
        // Assign a function to process each node.
        _get_node_process: impl Fn(NodeHandle) -> Box<dyn FnMut(ProcessContext)>,
        // Get the inherent delay of a node in the graph.
        get_node_delay: impl Fn(NodeHandle) -> u64,
    ) -> Result<Schedule, Error> {
        // First, we compute the delay compensation information and topological ordering.
        let mut comps = vec![]; // latency compensation requirements
        let mut latencies = HashMap::new(); // list of internal latencies.
        let mut order = Vec::with_capacity(self.nodes.len()); // the order of the graph
        let mut stack = Vec::with_capacity(self.nodes.len() * 2); // a stack used for recursion
        let _ = self.get_node(root)?; // check that the root exists
        stack.push((root.0, 0));
        // main driver
        'outer: while let Some((node, mut latency)) = stack.pop() {
            let this_node = if let Ok(n) = self.get_node(NodeHandle(node)) {
                n
            } else {
                // keep going if the node doesn't exist, not our problem. We should probably
                // add a warning that our graph has been partially corruped.
                continue;
            };
            // loop through the incoming edges to the node.
            for (src, _src_port, _dst_port) in &this_node.dependencies {
                // check to make sure the node exists before adding it to the order.
                // again, we don't care if the node doesn't exist, but we may want to add a warning.
                if let Ok(_) = self.get_node(NodeHandle(*src)) {
                    // if a latency has been assigned for a dependency, use it to
                    // compute the max latency of incoming data to this node. If it
                    // hasn't, recurse.
                    match latencies.get(src) {
                        Some(l) => {
                            latency = latency.max(*l);
                        }
                        None => {
                            stack.push((node, latency));
                            stack.push((*src, 0));
                            continue 'outer;
                        }
                    }
                }
            }
            // Topological ordering comes for free, since we walk the graph in topological order.
            order.push(NodeHandle(node));

            // now we need to walk the incoming edges again to compute latency compensation requires
            // for each edge.
            for (src, src_port, dst_port) in &this_node.dependencies {
                if let Ok(_) = self.get_node(NodeHandle(*src)) {
                    let compensation = latency - latencies.get(src).unwrap();
                    if compensation != 0 {
                        comps.push(((*src, *src_port), (node, *dst_port), compensation));
                    }
                }
            }
            // finally, we have our overall latency plus whatever internal delay our node imparts. Add
            // it to the list of internal latencies.
            latencies.insert(node, latency + get_node_delay(NodeHandle(node)));
        }

        // Each input port with a connection requires a buffer
        // Each output port with a connection requires a buffer
        //
        // if an outgoing edge has a delay compensation requirement, we assign a delay buffer.
        //
        // if an incoming edge shares endpoints with other incoming edges, we assign a sum buffer.
        //
        // otherwise, we assign simple buffers.
        todo!()
    }
}

/// The ProcessContext is used to implement processing at nodes along the graph.
pub struct ProcessContext<'a> {
    /// A slice of input buffers mapped to ports
    buffers: &'a mut [Box<dyn AudioBuffer>],
    inputs: &'a [(PortHandle, usize)],
    outputs: &'a [(PortHandle, usize)],
    /// The sample rate to evaluate the graph
    pub sample_rate: f64,
    /// The current buffer size to process
    pub buffer_size: usize,
}

pub struct Schedule {
    entries: Vec<ScheduleEntry>,
    buffers: Vec<Box<dyn AudioBuffer>>,
}

pub struct ScheduleEntry {
    process: Box<dyn FnMut(ProcessContext)>,
    inputs: Vec<(PortHandle, usize)>,
    outputs: Vec<(PortHandle, usize)>,
}

impl<'a> ProcessContext<'a> {
    /// Get an input buffer by name. Returns None if it cannot be found
    pub fn get_input(&self, port: &str) -> Option<RefBuffer<'_>> {
        let port = PortHandle(hash_str(port));
        self.inputs.iter().find_map(move |(p, i)| {
            if *p == port {
                Some(self.buffers[*i].as_ref_buffer())
            } else {
                None
            }
        })
    }
    /// Get an output buffer by name. Returns None if it cannot be found.
    pub fn get_output(&mut self, port: &str) -> Option<RefBufferMut<'_>> {
        let ProcessContext {
            buffers, outputs, ..
        } = self;
        let port = PortHandle(hash_str(port));
        for (p, i) in outputs.iter() {
            if *p == port {
                return Some((&mut buffers[*i]).as_ref_buffer_mut());
            }
        }
        None
    }
}

impl Schedule {
    pub fn eval(&mut self, mut playback_context: PlaybackContext) {
        let num_entries = self.entries.len();
        for entry in &mut self.entries[0..num_entries - 1] {
            (&mut entry.process)(ProcessContext {
                buffers: &mut self.buffers,
                inputs: &entry.inputs,
                outputs: &mut entry.outputs,
                sample_rate: playback_context.sample_rate,
                buffer_size: playback_context.buffer_size,
            });
        }
        let buffer_size = playback_context.buffer_size;
        match (self.entries.last_mut(), playback_context.get_buffer()) {
            (Some(entry), Ok(mut buffer)) => {
                if let Some((_, idx)) = entry.outputs.first() {
                    let ibuf = &mut self.buffers[*idx];
                    for (ich, och) in (0..ibuf.get_channel_config().count())
                        .zip(0..buffer.get_channel_config().count())
                    {
                        let (input, output) = (ibuf.get_channel(ich), buffer.get_channel_mut(och));
                        if let (Ok(input), Ok(output)) = (input, output) {
                            output[0..buffer_size].copy_from_slice(&input[0..buffer_size]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_buffer::channel_description;
    #[test]
    fn nodes() {
        let mut graph = Graph::default();
        let node0 = graph.add_node("node-0").expect("could not create node");
        let _ = graph
            .add_node("node-0")
            .expect_err("created duplicate nodes");
        graph.del_node(node0).expect("could not delete node");
        graph.del_node(node0).expect_err("deleted node twice");
    }

    #[test]
    fn ports() {
        let mut graph = Graph::default();
        let node = graph.add_node("node").expect("could not create node");
        let input = graph
            .add_port(Direction::Input, node, "input")
            .expect("could not create input port");
        let _output = graph
            .add_port(Direction::Output, node, "output")
            .expect("could not create output port");
        let _ = graph
            .add_port(Direction::Input, node, "input")
            .expect_err("created duplicate ports");
        let _ = graph
            .add_port(Direction::Output, node, "output")
            .expect_err("created duplicate ports");
        graph
            .del_port(Direction::Input, node, input)
            .expect("could not delete input node");
        graph
            .del_port(Direction::Output, node, input)
            .expect_err("deleted node that doesn't exist");
        graph
            .del_port(Direction::Input, node, input)
            .expect_err("deleted node twice");
    }

    #[test]
    fn connections() {
        let mut graph = Graph::default();
        let (node0, node1) = (
            graph.add_node("foo").expect("could not create foo"),
            graph.add_node("bar").expect("could not create bar"),
        );
        let output = graph
            .add_port(Direction::Output, node0, "foo.output")
            .expect("could not create foo.output");
        let input = graph
            .add_port(Direction::Input, node1, "bar.input")
            .expect("could not create bar.input");
        graph
            .configure_port(
                Direction::Output,
                node0,
                output,
                channel_description::stereo(),
            )
            .expect("could not configure foo.output as stereo");
        graph
            .configure_port(
                Direction::Input,
                node1,
                input,
                channel_description::stereo(),
            )
            .expect("could not configure bar.input as stereo");
        graph
            .connect((node0, output), (node1, input))
            .expect("could not connect foo.output -> bar.input");
        graph
            .configure_port(Direction::Input, node1, input, channel_description::mono())
            .expect("could not reconfigure bar.input as mono");
        graph
            .connect((node0, output), (node1, input))
            .expect_err("connected nodes with invalid configurations");
    }
}
