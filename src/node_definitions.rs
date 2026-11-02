//! Stable authoring metadata. Legacy nodes continue to evaluate as RGB;
//! this is a compatibility adapter, not a second graph execution engine.
use crate::NodeKind;

/// Logical socket semantics. Unmigrated nodes remain Color because the old
/// evaluator transports RGB even for roughness/height and grayscale fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Scalar,
    Mask,
    Color,
    Vector2,
    Vector3,
    Gradient,
    Curve,
}

impl SocketType {
    /// Explicit, one-way lossless/meaningful conversions only.
    pub fn can_feed(self, expected:Self)->bool {
        self==expected || matches!((self,expected),
            (Self::Scalar,Self::Color) |
            (Self::Mask,Self::Scalar) |
            (Self::Mask,Self::Color))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub data_type: SocketType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParameterControl {
    Float {min:f32,max:f32,default:f32,unit:&'static str},
    Integer {min:u32,max:u32,step:u32,default:u32},
    Seed {default:u32},
    Color {default:[u8;3]},
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParameterDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub control: ParameterControl,
}

const THRESHOLD_MASK: &[ParameterDefinition]=&[
    ParameterDefinition{id:"threshold",label:"Threshold",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"normalized"}},
    ParameterDefinition{id:"softness",label:"Edge softness",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.0,unit:"normalized"}},
];
const SCALAR: &[ParameterDefinition]=&[
    ParameterDefinition{id:"value",label:"Value",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"normalized"}},
];
const LEVELS: &[ParameterDefinition]=&[
    ParameterDefinition{id:"black",label:"Black point",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.0,unit:"normalized"}},
    ParameterDefinition{id:"white",label:"White point",control:ParameterControl::Float{
        min:0.0,max:1.0,default:1.0,unit:"normalized"}},
    ParameterDefinition{id:"gamma",label:"Gamma",control:ParameterControl::Float{
        min:0.1,max:5.0,default:1.0,unit:"exponent"}},
];
const NOISE: &[ParameterDefinition]=&[
    ParameterDefinition{id:"scale",label:"Frequency",control:ParameterControl::Integer{
        min:1,max:64,step:1,default:6}},
    ParameterDefinition{id:"octaves",label:"Octaves",control:ParameterControl::Integer{
        min:1,max:6,step:1,default:4}},
    ParameterDefinition{id:"seed",label:"Seed",control:ParameterControl::Seed{default:42}},
];
const CHECKER: &[ParameterDefinition]=&[
    ParameterDefinition{id:"tiles",label:"Tile count",control:ParameterControl::Integer{
        min:2,max:64,step:2,default:8}},
];
const BRICKS: &[ParameterDefinition]=&[
    ParameterDefinition{id:"columns",label:"Columns",control:ParameterControl::Integer{
        min:2,max:64,step:1,default:8}},
    ParameterDefinition{id:"rows",label:"Rows",control:ParameterControl::Integer{
        min:2,max:64,step:2,default:12}},
    ParameterDefinition{id:"mortar",label:"Mortar width",control:ParameterControl::Float{
        min:0.0,max:0.4,default:0.12,unit:"normalized"}},
];
const VORONOI: &[ParameterDefinition]=&[
    ParameterDefinition{id:"cells",label:"Cell count",control:ParameterControl::Integer{
        min:2,max:32,step:1,default:7}},
    ParameterDefinition{id:"seed",label:"Seed",control:ParameterControl::Seed{default:17}},
];
const STRATA: &[ParameterDefinition]=&[
    ParameterDefinition{id:"bands",label:"Bands",control:ParameterControl::Integer{
        min:1,max:64,step:1,default:14}},
    ParameterDefinition{id:"warp",label:"Warp",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.25,unit:"normalized"}},
    ParameterDefinition{id:"seed",label:"Seed",control:ParameterControl::Seed{default:11}},
];
const COLORIZE: &[ParameterDefinition]=&[
    ParameterDefinition{id:"dark",label:"Dark color",control:ParameterControl::Color{
        default:[24,37,54]}},
    ParameterDefinition{id:"light",label:"Light color",control:ParameterControl::Color{
        default:[204,175,130]}},
];
const BLEND: &[ParameterDefinition]=&[
    ParameterDefinition{id:"factor",label:"Mix",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"normalized"}},
];
const COLOR: &[ParameterDefinition]=&[
    ParameterDefinition{id:"rgb",label:"Color",control:ParameterControl::Color{
        default:[98,152,203]}},
];
const LIBRARY: &[ParameterDefinition]=&[
    ParameterDefinition{id:"amount",label:"Amount / scale",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"normalized"}},
    ParameterDefinition{id:"seed",label:"Seed",control:ParameterControl::Seed{default:42}},
];

/// Params are authored data, not a dynamic runtime-typed property map.
pub fn parameters(kind:&NodeKind)-> &'static [ParameterDefinition] {
    match kind {
        NodeKind::Scalar{..}=>SCALAR,
        NodeKind::Levels{..}=>LEVELS,
        NodeKind::ThresholdMask{..}=>THRESHOLD_MASK,
        NodeKind::Noise{..}=>NOISE,
        NodeKind::Checker{..}=>CHECKER,
        NodeKind::Bricks{..}=>BRICKS,
        NodeKind::Voronoi{..}=>VORONOI,
        NodeKind::Strata{..}=>STRATA,
        NodeKind::Colorize{..}=>COLORIZE,
        NodeKind::Blend{..}=>BLEND,
        NodeKind::Color{..}=>COLOR,
        NodeKind::Library{..}=>LIBRARY,
        _=>&[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversion_rules_reject_ambiguous_color_scalar_casts() {
        assert!(SocketType::Scalar.can_feed(SocketType::Color));
        assert!(SocketType::Mask.can_feed(SocketType::Color));
        assert!(!SocketType::Color.can_feed(SocketType::Scalar));
        assert!(!SocketType::Vector2.can_feed(SocketType::Color));
        assert!(!SocketType::Gradient.can_feed(SocketType::Curve));
    }

    #[test]
    fn all_legacy_parameterized_kinds_expose_expected_ids() {
        use crate::node_library::LibraryNode;
        let cases = [
            (NodeKind::Noise{scale:6,octaves:4,seed:42}, vec!["scale","octaves","seed"]),
            (NodeKind::Checker{tiles:8}, vec!["tiles"]),
            (NodeKind::Bricks{columns:8,rows:12,mortar:0.12},vec!["columns","rows","mortar"]),
            (NodeKind::Voronoi{cells:7,seed:17},vec!["cells","seed"]),
            (NodeKind::Strata{bands:14,warp:0.25,seed:11},vec!["bands","warp","seed"]),
            (NodeKind::Colorize{dark:[24,37,54],light:[204,175,130]},vec!["dark","light"]),
            (NodeKind::Blend{factor:0.5},vec!["factor"]),
            (NodeKind::Color{rgb:[98,152,203]},vec!["rgb"]),
            (NodeKind::Library{operation:LibraryNode::Warp,amount:0.5,seed:42},vec!["amount","seed"]),
        ];
        for (kind,ids) in cases {
            assert_eq!(parameters(&kind).iter().map(|p|p.id).collect::<Vec<_>>(),ids);
        }
        for op in LibraryNode::ALL {
            let kind=NodeKind::Library{operation:*op,amount:0.5,seed:42};
            assert_eq!(parameters(&kind).len(),2,"{op:?}");
        }
        assert!(matches!(parameters(&NodeKind::Checker{tiles:8})[0].control,
            ParameterControl::Integer{step:2,..}));
        assert!(matches!(parameters(&NodeKind::Bricks{columns:8,rows:12,mortar:0.1})[1].control,
            ParameterControl::Integer{step:2,..}));
    }

    #[test]
    fn level_parameters_have_stable_ids_and_explicit_ranges() {
        let params=parameters(&NodeKind::Levels{black:0.0,white:1.0,gamma:1.0});
        assert_eq!(params.iter().map(|p|p.id).collect::<Vec<_>>(),
            vec!["black","white","gamma"]);
        assert!(matches!(params[2].control,ParameterControl::Float{
            min:0.1,max:5.0,default:1.0,..
        }));
    }
}
