//! Stable authoring metadata. Legacy nodes continue to evaluate as RGB;
//! this is a compatibility adapter, not a second graph execution engine.
use crate::NodeKind;
use serde::{Deserialize,Serialize};

/// Logical socket semantics. Unmigrated nodes remain Color because the old
/// evaluator transports RGB even for roughness/height and grayscale fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SocketType {
    Scalar,
    Mask,
    /// Signed normalized-UV distance; no implicit cast to Mask or Color.
    Distance,
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
    ColorStops {min:u8,max:u8,default:u8},
    CurvePoints {min:u8,max:u8,default:u8},
    Choice {options:&'static [&'static str],default:&'static str},
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParameterDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub control: ParameterControl,
}

const SHAPE_SCATTER: &[ParameterDefinition]=&[
    ParameterDefinition{id:"cells",label:"Cells per axis",control:ParameterControl::Integer{
        min:2,max:32,step:1,default:10}},
    ParameterDefinition{id:"scale",label:"Stamp scale",control:ParameterControl::Float{
        min:0.1,max:0.85,default:0.8,unit:"cell"}},
    ParameterDefinition{id:"jitter",label:"Position jitter",control:ParameterControl::Float{
        min:0.0,max:0.25,default:0.2,unit:"cell"}},
    ParameterDefinition{id:"rotation",label:"Rotation variation",control:ParameterControl::Float{
        min:0.0,max:1.0,default:1.0,unit:"turn"}},
    ParameterDefinition{id:"brightness",label:"Brightness variation",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.3,unit:"normalized"}},
    ParameterDefinition{id:"seed",label:"Seed",control:ParameterControl::Seed{default:55}},
];
const SDF_CIRCLE: &[ParameterDefinition]=&[
    ParameterDefinition{id:"center_x",label:"Center X",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"tile UV"}},
    ParameterDefinition{id:"center_y",label:"Center Y",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"tile UV"}},
    ParameterDefinition{id:"radius",label:"Radius",control:ParameterControl::Float{
        min:0.001,max:0.5,default:0.22,unit:"tile UV"}},
];
const SDF_ELLIPSE: &[ParameterDefinition]=&[
    ParameterDefinition{id:"center_x",label:"Center X",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"tile UV"}},
    ParameterDefinition{id:"center_y",label:"Center Y",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"tile UV"}},
    ParameterDefinition{id:"radius_x",label:"Radius X",control:ParameterControl::Float{
        min:0.001,max:0.5,default:0.28,unit:"tile UV"}},
    ParameterDefinition{id:"radius_y",label:"Radius Y",control:ParameterControl::Float{
        min:0.001,max:0.5,default:0.14,unit:"tile UV"}},
];
const SDF_CONTOUR_WARP: &[ParameterDefinition]=&[
    ParameterDefinition{id:"frequency",label:"Noise frequency",control:ParameterControl::Integer{
        min:1,max:64,step:1,default:8}},
    ParameterDefinition{id:"amount",label:"UV warp amount",control:ParameterControl::Float{
        min:0.0,max:0.25,default:0.05,unit:"tile UV"}},
    ParameterDefinition{id:"seed",label:"Seed",control:ParameterControl::Seed{default:57}},
];
const SDF_BEZIER: &[ParameterDefinition]=&[
    ParameterDefinition{id:"point_0_x",label:"Anchor X",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.2,unit:"tile UV"}},
    ParameterDefinition{id:"point_0_y",label:"Anchor Y",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"tile UV"}},
    ParameterDefinition{id:"point_1_x",label:"Handle A X",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.35,unit:"tile UV"}},
    ParameterDefinition{id:"point_1_y",label:"Handle A Y",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.2,unit:"tile UV"}},
    ParameterDefinition{id:"point_2_x",label:"Handle B X",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.65,unit:"tile UV"}},
    ParameterDefinition{id:"point_2_y",label:"Handle B Y",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.8,unit:"tile UV"}},
    ParameterDefinition{id:"point_3_x",label:"Endpoint X",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.8,unit:"tile UV"}},
    ParameterDefinition{id:"point_3_y",label:"Endpoint Y",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"tile UV"}},
    ParameterDefinition{id:"start_radius",label:"Start radius",control:ParameterControl::Float{
        min:0.001,max:0.25,default:0.07,unit:"tile UV"}},
    ParameterDefinition{id:"end_radius",label:"End radius",control:ParameterControl::Float{
        min:0.001,max:0.25,default:0.02,unit:"tile UV"}},
    ParameterDefinition{id:"segments",label:"Curve segments",control:ParameterControl::Integer{
        min:8,max:64,step:1,default:24}},
    ParameterDefinition{id:"tiled",label:"UV tiling",control:ParameterControl::Choice{
        options:&["Tile","Clamp"],default:"Tile"}},
];
const SDF_CAPSULE: &[ParameterDefinition]=&[
    ParameterDefinition{id:"start_x",label:"Start X",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.30,unit:"tile UV"}},
    ParameterDefinition{id:"start_y",label:"Start Y",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"tile UV"}},
    ParameterDefinition{id:"end_x",label:"End X",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.70,unit:"tile UV"}},
    ParameterDefinition{id:"end_y",label:"End Y",control:ParameterControl::Float{
        min:0.0,max:1.0,default:0.5,unit:"tile UV"}},
    ParameterDefinition{id:"radius",label:"Radius",control:ParameterControl::Float{
        min:0.001,max:0.5,default:0.07,unit:"tile UV"}},
];
const SDF_BOOLEAN: &[ParameterDefinition]=&[
    ParameterDefinition{id:"operation",label:"Boolean operation",control:ParameterControl::Choice{
        options:&["Union","Intersection","Subtract"],default:"Union"}},
    ParameterDefinition{id:"smoothing",label:"Smooth width",control:ParameterControl::Float{
        min:0.0,max:0.25,default:0.0,unit:"tile UV"}},
];
const SDF_TO_MASK: &[ParameterDefinition]=&[
    ParameterDefinition{id:"edge_width",label:"Coverage width",control:ParameterControl::Float{
        min:0.0,max:0.25,default:0.01,unit:"tile UV"}},
];
const CURVE_REMAP: &[ParameterDefinition]=&[
    ParameterDefinition{id:"curve",label:"Control points",control:ParameterControl::CurvePoints{
        min:2,max:8,default:4}},
];
const COLOR_RAMP: &[ParameterDefinition]=&[
    ParameterDefinition{id:"ramp",label:"Color stops",control:ParameterControl::ColorStops{
        min:2,max:8,default:4}},
];
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
        NodeKind::ColorRamp{..}=>COLOR_RAMP,
        NodeKind::CurveRemap{..}=>CURVE_REMAP,
        NodeKind::SdfCircle{..}=>SDF_CIRCLE,
        NodeKind::SdfEllipse{..}=>SDF_ELLIPSE,
        NodeKind::SdfCapsule{..}=>SDF_CAPSULE,
        NodeKind::SdfBezierStroke{..}=>SDF_BEZIER,
        NodeKind::SdfContourWarp{..}=>SDF_CONTOUR_WARP,
        NodeKind::SdfCombine{..}=>SDF_BOOLEAN,
        NodeKind::SdfToMask{..}=>SDF_TO_MASK,
        NodeKind::ShapeScatter{..}=>SHAPE_SCATTER,
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
        assert!(!SocketType::Distance.can_feed(SocketType::Mask));
        assert!(!SocketType::Distance.can_feed(SocketType::Color));
        assert!(!SocketType::Scalar.can_feed(SocketType::Distance));
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
    fn all_catalog_graphs_have_valid_unique_parameter_definitions() {
        use crate::presets::builtin_presets;
        use crate::VoxelSettings;
        let presets=builtin_presets().unwrap();
        assert_eq!(presets.len(),1067);
        for preset in presets {
            let graph=preset.build_graph(VoxelSettings::default());
            graph.validate().unwrap();
            for node in &graph.nodes {
                let mut ids=std::collections::HashSet::new();
                for param in node.kind.parameters() {
                    assert!(ids.insert(param.id),"duplicate {} in {}",
                        param.id,node.kind.type_id());
                    assert!(!param.label.is_empty());
                    match param.control {
                        ParameterControl::Float{min,max,default,..} => {
                            assert!(min.is_finite()&&max.is_finite()&&default.is_finite());
                            assert!(min<=default&&default<=max);
                        }
                        ParameterControl::Integer{min,max,step,default} => {
                            assert!(step>0&&min<=default&&default<=max);
                            assert_eq!((default-min)%step,0);
                        }
                        ParameterControl::Seed{..}|ParameterControl::Color{..}=>{},
                        ParameterControl::Choice{options,default}=> {
                            assert!(!options.is_empty()&&options.contains(&default));
                        },
                        ParameterControl::ColorStops{min,max,default}
                        | ParameterControl::CurvePoints{min,max,default} => {
                            assert!(min>=2&&min<=default&&default<=max&&max<=8);
                        }
                    }
                }
                for index in 0..node.kind.input_count() {
                    let port=node.kind.input_definition(index).unwrap();
                    assert!(!port.id.is_empty());
                    assert!(!port.label.is_empty());
                }
                if !matches!(node.kind,NodeKind::Output) {
                    assert!(node.kind.output_definition().is_some());
                }
            }
        }
    }

    #[test]
    fn contour_warp_metadata_stays_signed_distance() {
        let node=NodeKind::SdfContourWarp{
            frequency:8,amount:0.05,seed:57,
        };
        assert_eq!(node.input_type(0),Some(SocketType::Distance));
        assert_eq!(node.output_type(),SocketType::Distance);
        assert_eq!(parameters(&node).iter().map(|p|p.id).collect::<Vec<_>>(),
            vec!["frequency","amount","seed"]);
    }

    #[test]
    fn cubic_stroke_exposes_four_handles_and_taper_metadata() {
        let node=NodeKind::SdfBezierStroke{
            points:[[0.2,0.5],[0.35,0.2],[0.65,0.8],[0.8,0.5]],
            start_radius:0.07,end_radius:0.02,segments:24,tiled:true,
        };
        let params=parameters(&node);
        assert_eq!(params.len(),12);
        assert_eq!(params[0].id,"point_0_x");
        assert_eq!(params[7].id,"point_3_y");
        assert_eq!(params[8].id,"start_radius");
        assert_eq!(params[11].id,"tiled");
        assert_eq!(node.output_type(),SocketType::Distance);
    }

    #[test]
    fn sdf_nodes_expose_distance_types_and_bounded_parameter_metadata() {
        let circle=NodeKind::SdfCircle{center:[0.5,0.5],radius:0.22};
        let boolean=NodeKind::SdfCombine{operation:crate::SdfBoolean::Union,
            smoothing:0.0};
        let mask=NodeKind::SdfToMask{edge_width:0.01};
        assert_eq!(circle.output_type(),SocketType::Distance);
        assert_eq!(boolean.input_type(0),Some(SocketType::Distance));
        assert_eq!(boolean.input_type(1),Some(SocketType::Distance));
        assert_eq!(mask.input_type(0),Some(SocketType::Distance));
        assert_eq!(mask.output_type(),SocketType::Mask);
        assert_eq!(parameters(&circle).iter().map(|p|p.id).collect::<Vec<_>>(),
            vec!["center_x","center_y","radius"]);
        assert_eq!(parameters(&mask)[0].id,"edge_width");
        assert_eq!(parameters(&boolean)[0].id,"operation");
    }

    #[test]
    fn curve_remap_metadata_is_typed_and_bounded() {
        let kind=NodeKind::CurveRemap{curve:crate::CurveRemap::default()};
        let parameters=parameters(&kind);
        assert_eq!(parameters.len(),1);
        assert_eq!(parameters[0].id,"curve");
        assert!(matches!(parameters[0].control,ParameterControl::CurvePoints{
            min:2,max:8,default:4}));
    }

    #[test]
    fn color_ramp_metadata_is_typed_and_bounded() {
        let kind=NodeKind::ColorRamp{ramp:crate::ColorRamp::default()};
        let params=parameters(&kind);
        assert_eq!(params.len(),1);
        assert_eq!(params[0].id,"ramp");
        assert!(matches!(params[0].control,ParameterControl::ColorStops{
            min:2,max:8,default:4}));
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
