//! Resolve nested function calls into the same flat Graph used by all
//! existing material baker, cache, asset and PBR export code paths.
use crate::{Graph,Link,NodeKind};
use crate::material_functions::{MaterialFunctionLibrary,MaterialFunctionInstance};
use std::collections::HashMap;

const MAX_NESTING_DEPTH:usize=8;
const MAX_RESOLVED_NODES:usize=2048;

pub fn resolve(library:&MaterialFunctionLibrary,
    reference:&MaterialFunctionInstance)->Result<Graph,String> {
    let mut stack=Vec::new();
    resolve_nested(library,reference,&mut stack)
}

fn resolve_nested(library:&MaterialFunctionLibrary,
    reference:&MaterialFunctionInstance,
    stack:&mut Vec<(String,u32)>)->Result<Graph,String> {
    reference.validate()?;
    if stack.len()>=MAX_NESTING_DEPTH {
        return Err("Material function nesting exceeds eight levels".into());
    }
    let identifier=(reference.function_id.clone(),reference.version);
    if stack.contains(&identifier) {
        return Err("Material function recursion / dependency cycle detected".into());
    }
    let function=library.functions.iter()
        .find(|f|f.id==reference.function_id&&f.version==reference.version)
        .ok_or("Material function version not present in authoring library")?;
    stack.push(identifier);
    let result=(|| {
        let mut graph=function.instantiate(reference)?;
        for call in &function.calls {
            let child=resolve_nested(library,&call.reference,stack)?;
            splice_child_albedo(&mut graph,call.placeholder_node_id,child)?;
        }
        graph.validate()?;
        Ok(graph)
    })();
    stack.pop();
    result
}

/// Replace an authored Color placeholder with the child's Albedo *source*.
/// Every child node receives a new unique ID, and the child's Material
/// Output is discarded; no new runtime node kind, shader or evaluator exists.
fn splice_child_albedo(parent:&mut Graph,placeholder_id:u64,
    child:Graph)->Result<(),String> {
    parent.validate()?;
    child.validate()?;
    let placeholder=parent.node(placeholder_id)
        .ok_or("Nested function placeholder missing at instantiation")?;
    if !matches!(placeholder.kind,NodeKind::Color{..}) {
        return Err("Nested function placeholder is not a Color node".into());
    }
    let output=child.nodes.iter().find(|n|matches!(n.kind,NodeKind::Output))
        .expect("validated child has exactly one Output").id;
    let source=child.links.iter().find(|link|
        link.to==output && link.input==0)
        .ok_or("Nested material function must connect its Albedo output")?.from;
    let new_nodes=child.nodes.len()-1;
    let total=parent.nodes.len().checked_sub(1)
        .and_then(|n|n.checked_add(new_nodes))
        .ok_or("Resolved material function node count overflow")?;
    if total>MAX_RESOLVED_NODES {
        return Err("Resolved material function exceeds 2048 nodes".into());
    }
    let mut next=parent.nodes.iter().map(|n|n.id).max().unwrap_or(0);
    let mut remap=HashMap::new();
    for node in child.nodes.iter().filter(|n|n.id!=output) {
        next=next.checked_add(1)
            .ok_or("Nested material function node ID overflow")?;
        remap.insert(node.id,next);
        let mut clone=node.clone();
        clone.id=next;
        parent.nodes.push(clone);
    }
    let source_id=*remap.get(&source)
        .ok_or("Nested material function Albedo source missing")?;
    // Preserve the placeholder's outgoing destination port indices.
    for link in &mut parent.links {
        if link.from==placeholder_id {link.from=source_id;}
    }
    parent.nodes.retain(|node|node.id!=placeholder_id);
    for link in child.links.iter().filter(|link|link.to!=output) {
        parent.links.push(Link{
            from:*remap.get(&link.from)
                .ok_or("Nested material function source link missing")?,
            to:*remap.get(&link.to)
                .ok_or("Nested material function destination link missing")?,
            input:link.input,
        });
    }
    parent.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeKind};
    use crate::foliage_materials::{FoliageMaterials,MaterialRole};
    use crate::material_functions::{
        MaterialFunction,MaterialFunctionCall,FunctionOverride,
    };

    fn inst(id:&str,version:u32,scale:Option<u32>)->MaterialFunctionInstance {
        MaterialFunctionInstance{
            function_id:id.into(),version,
            overrides:scale.map(|value|vec![FunctionOverride{
                key:"frequency".into(),value,
            }]).unwrap_or_default(),
        }
    }

    fn with_placeholder(id:&str,child:&str)->MaterialFunction {
        let mut graph=Graph::default();
        let placeholder=graph.add_node(NodeKind::Color{rgb:[255,0,0]},[40.0,40.0]);
        graph.connect(placeholder,2,0).unwrap();
        let mut parent=MaterialFunction::from_graph(id.into(),1,graph).unwrap();
        parent.calls.push(MaterialFunctionCall{
            placeholder_node_id:placeholder,
            reference:inst(child,1,None),
        });
        parent.validate().unwrap();
        parent
    }

    #[test]
    fn nested_call_expands_into_existing_graph_and_matches_child_pixels() {
        let mut library=MaterialFunctionLibrary::default();
        let child=MaterialFunction::from_graph("bark_detail".into(),1,
            Graph::default()).unwrap();
        library.insert(child).unwrap();
        library.insert(with_placeholder("trunk_bark","bark_detail")).unwrap();
        library.validate().unwrap();
        let resolved=library.resolve(&inst("trunk_bark",1,None)).unwrap();
        let original=library.resolve(&inst("bark_detail",1,None)).unwrap();
        assert_eq!(resolved.render_rgba(32,32).unwrap(),
            original.render_rgba(32,32).unwrap());
        assert_eq!(resolved.nodes.iter()
            .filter(|n|matches!(n.kind,NodeKind::Output)).count(),1);
        assert_eq!(resolved.nodes.len(),4,
            "parent Noise and Output plus remapped child Noise");
    }

    #[test]
    fn recursive_three_level_functions_and_overrides_remain_portable() {
        let mut library=MaterialFunctionLibrary::default();
        library.insert(MaterialFunction::from_graph("noise".into(),1,
            Graph::default()).unwrap()).unwrap();
        let mut middle=with_placeholder("detail","noise");
        middle.calls[0].reference=inst("noise",1,Some(23));
        library.insert(middle).unwrap();
        library.insert(with_placeholder("bark","detail")).unwrap();
        library.validate().unwrap();
        let graph=library.resolve(&inst("bark",1,None)).unwrap();
        let expected=library.resolve(&inst("noise",1,Some(23))).unwrap();
        assert_eq!(graph.render_rgba(16,16).unwrap(),
            expected.render_rgba(16,16).unwrap());
        let mut materials=FoliageMaterials::default();
        materials.apply_function(MaterialRole::Bark,&library,
            inst("bark",1,None)).unwrap();
        let baked=materials.bake_bark(32,[100,70,45]).unwrap();
        let serialized=serde_json::to_string(&materials).unwrap();
        drop(library);
        let restored:FoliageMaterials=serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored.bake_bark(32,[100,70,45]).unwrap(),baked);
    }

    #[test]
    fn direct_and_indirect_cycles_and_missing_versions_fail() {
        let mut library=MaterialFunctionLibrary::default();
        library.insert(with_placeholder("a","b")).unwrap();
        library.insert(with_placeholder("b","a")).unwrap();
        assert!(library.resolve(&inst("a",1,None)).unwrap_err()
            .contains("cycle"));
        assert!(library.validate().is_err());
        let mut self_cycle=MaterialFunctionLibrary::default();
        self_cycle.insert(with_placeholder("a","a")).unwrap();
        assert!(self_cycle.resolve(&inst("a",1,None)).is_err());
        let mut missing=MaterialFunctionLibrary::default();
        missing.insert(with_placeholder("a","does_not_exist")).unwrap();
        assert!(missing.resolve(&inst("a",1,None)).is_err());
    }

    #[test]
    fn malformed_placeholder_duplicate_calls_and_missing_child_output_fail() {
        let mut library=MaterialFunctionLibrary::default();
        library.insert(MaterialFunction::from_graph("child".into(),1,
            Graph::default()).unwrap()).unwrap();
        let mut function=with_placeholder("parent","child");
        let duplicate=function.calls[0].clone();
        function.calls.push(duplicate);
        assert!(function.validate().is_err());
        function.calls.pop();
        function.calls[0].placeholder_node_id=1; // Noise, not Color
        assert!(function.validate().is_err());
        let mut child=Graph::default();
        child.disconnect(2,0);
        let mut broken=MaterialFunctionLibrary::default();
        broken.insert(MaterialFunction::from_graph("child".into(),1,
            child).unwrap()).unwrap();
        broken.insert(with_placeholder("parent","child")).unwrap();
        assert!(broken.resolve(&inst("parent",1,None)).unwrap_err()
            .contains("Albedo"));
    }

    #[test]
    fn calling_same_child_twice_is_deterministic_and_valid() {
        let mut library=MaterialFunctionLibrary::default();
        library.insert(MaterialFunction::from_graph("child".into(),1,
            Graph::default()).unwrap()).unwrap();
        let mut parent=Graph::default();
        let a=parent.add_node(NodeKind::Color{rgb:[255,0,0]},[20.0,0.0]);
        let b=parent.add_node(NodeKind::Color{rgb:[0,0,255]},[30.0,0.0]);
        parent.connect(a,2,0).unwrap();
        parent.connect(b,2,1).unwrap();
        let mut function=MaterialFunction::from_graph("parent".into(),1,parent)
            .unwrap();
        function.calls=vec![
            MaterialFunctionCall{placeholder_node_id:a,reference:inst("child",1,None)},
            MaterialFunctionCall{placeholder_node_id:b,reference:inst("child",1,Some(16))},
        ];
        library.insert(function).unwrap();
        let resolved=library.resolve(&inst("parent",1,None)).unwrap();
        resolved.validate().unwrap();
        assert_eq!(resolved.render_rgba(32,32).unwrap(),
            library.resolve(&inst("child",1,None))
                .unwrap().render_rgba(32,32).unwrap());
        assert_eq!(resolved.render_output_channel_rgba(1,32,32).unwrap(),
            library.resolve(&inst("child",1,Some(16)))
                .unwrap().render_rgba(32,32).unwrap());
    }
}
