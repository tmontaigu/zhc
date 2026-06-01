use super::*;

/// Text element representing an operation input port.
pub type OpInputPort = TextBox<OpInputPortClass>;

/// Horizontal collection of input ports.
pub type OpInputs = HStack<OpInputPort, OpInputsClass>;

/// Text element representing an operation body.
pub type OpBody = TextBox<OpBodyClass>;

/// Text element representing an operation comment.
pub type OpComment = TextBox<OpCommentClass>;

/// Text element representing an operation output port.
pub type OpOutputPort = TextBox<OpOutputPortClass>;

/// Horizontal collection of output ports.
pub type OpOutputs = HStack<OpOutputPort, OpOutputsClass>;

/// Input operation: body + optional comment + outputs.
pub type InputOp =
    V4<OpBody, Optional<OpComment>, Optional<Box<dyn DynamicElement>>, OpOutputs, InputOpClass>;

/// Standard operation: inputs + body + optional comment + outputs.
pub type Op = V5<
    OpInputs,
    OpBody,
    Optional<OpComment>,
    Optional<Box<dyn DynamicElement>>,
    OpOutputs,
    OpClass,
>;

/// Effect operation: inputs + body + optional comment.
pub type EffectOp =
    V4<OpInputs, OpBody, Optional<OpComment>, Optional<Box<dyn DynamicElement>>, EffectOpClass>;

/// Empty placeholder element for missing nodes.
pub type Dummy = Empty<DummyClass>;

/// Text element representing a group input boundary port.
pub type GroupInputPort = Empty<GroupInputPortClass>;

/// Horizontal collection of group input ports.
pub type GroupInputs = HStack<GroupInputPort, GroupInputsClass>;

/// Text element representing a group output boundary port.
pub type GroupOutputPort = Empty<GroupOutputPortClass>;

/// Horizontal collection of group output ports.
pub type GroupOutputs = HStack<GroupOutputPort, GroupOutputsClass>;

/// Text element representing a group title.
pub type GroupTitle = TextBox<GroupTitleClass>;

/// Group element containing nested vertices with boundary ports.
pub struct Group(pub V4<GroupTitle, GroupInputs, GroupContent, GroupOutputs, GroupClass>);

impl SceneElement for Group {
    fn get_size(&self) -> zhc_utils::graphics::Size {
        self.0.get_size()
    }

    fn get_frame(&self) -> zhc_utils::graphics::Frame {
        self.0.get_frame()
    }

    fn get_variable_cell(&self) -> VariableCell {
        self.0.get_variable_cell()
    }
}

impl SceneSolver for Group {
    fn solve_size(&mut self) {
        self.0.solve_size();
    }

    fn solve_frame(&mut self, available: zhc_utils::graphics::Frame) {
        self.0.solve_frame(available);
    }
}

impl crate::visualization::svg::Renderable for Group {
    fn render(&self) -> Vec<crate::visualization::svg::SvgElement> {
        self.0.render()
    }
}

/// Runtime-polymorphic node payload (op, group, dummy, boundary port).
pub type NodeContent = D7<InputOp, Op, EffectOp, Dummy, Group, GroupInputPort, GroupOutputPort>;
pub use D7::E1 as NodeInputOpVar;
pub use D7::E2 as NodeOpVar;
pub use D7::E3 as NodeEffectOpVar;
pub use D7::E4 as NodeDummyVar;
pub use D7::E5 as NodeGroupVar;
pub use D7::E6 as NodeGroupInputPortVar;
pub use D7::E7 as NodeGroupOutputPortVar;

/// Node content tagged with its `OpId`; renders inside a `<g id="node-{opid}" class="node">`.
pub struct Node {
    pub id: crate::OpId,
    pub content: NodeContent,
}

impl SceneElement for Node {
    fn get_size(&self) -> zhc_utils::graphics::Size {
        self.content.get_size()
    }

    fn get_frame(&self) -> zhc_utils::graphics::Frame {
        self.content.get_frame()
    }

    fn get_variable_cell(&self) -> VariableCell {
        self.content.get_variable_cell()
    }
}

impl SceneSolver for Node {
    fn solve_size(&mut self) {
        self.content.solve_size();
    }

    fn solve_frame(&mut self, available: zhc_utils::graphics::Frame) {
        self.content.solve_frame(available);
    }
}

impl crate::visualization::svg::Renderable for Node {
    fn render(&self) -> Vec<crate::visualization::svg::SvgElement> {
        vec![crate::visualization::svg::SvgElement::Group {
            elements: self.content.render(),
            transform: None,
            id: Some(format!("node-{}", self.id.0)),
            class: Some("node".into()),
        }]
    }
}

/// Horizontal row of nodes forming a diagram layer.
pub type Layer = HStack<Node, LayerClass>;

pub type LayerSeparator = Spacer<LayerSpacerClass>;

pub type LayerMember = D2<Layer, LayerSeparator>;
pub use D2::E1 as LayerMemberLayer;
pub use D2::E2 as LayerMemberSeparator;

/// All the diagram layers
pub type Layers = VStack<LayerMember, LayersClass>;

/// Content inside a group element (uses smaller padding/spacing than top-level Vertices).
pub type GroupContent = Layers;

/// Collection of curves connecting nodes.
pub type Curves = Inert<Bag<Curve>>;

/// Root scene graph: layers with curves overlay.
pub type Scene = Z2<Layers, Curves>;
