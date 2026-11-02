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
    Integer {min:u32,max:u32,default:u32},
    Seed {default:u32},
    Color {default:[u8;3]},
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParameterDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub control: ParameterControl,
}

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
        min:1,max:64,default:6}},
    ParameterDefinition{id:"octaves",label:"Octaves",control:ParameterControl::Integer{
        min:1,max:6,default:4}},
    ParameterDefinition{id:"seed",label:"Seed",control:ParameterControl::Seed{default:42}},
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
        NodeKind::Noise{..}=>NOISE,
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
    fn level_parameters_have_stable_ids_and_explicit_ranges() {
        let params=parameters(&NodeKind::Levels{black:0.0,white:1.0,gamma:1.0});
        assert_eq!(params.iter().map(|p|p.id).collect::<Vec<_>>(),
            vec!["black","white","gamma"]);
        assert!(matches!(params[2].control,ParameterControl::Float{
            min:0.1,max:5.0,default:1.0,..
        }));
    }
}
