//! Portable versioned material graph templates with explicit instance overrides.
//! Templates resolve into the existing Graph when assigned: tree/asset runtime
//! and the foliage bake cache never depend on finding a live function library.
use crate::{Graph, NodeKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_FUNCTIONS:usize=128;
const MAX_PARAMETERS:usize=16;
const MAX_NESTED_CALLS:usize=16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FunctionParameterKind { NoiseScale, NestedNoiseScale }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionParameter {
    /// Stable key used by serialized instance overrides.
    pub key:String,
    pub node_id:u64,
    pub kind:FunctionParameterKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionOverride {
    pub key:String,
    pub value:u32,
}

/// A function reference is fixed to an exact version. The resolved graph
/// snapshot is persisted separately in FoliageMaterials to keep assets portable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialFunctionInstance {
    pub function_id:String,
    pub version:u32,
    #[serde(default)]
    pub overrides:Vec<FunctionOverride>,
}

fn valid_key(value:&str)->bool {
    !value.is_empty() && value.len()<=64 &&
        value.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'||b==b'-')
}

impl MaterialFunctionInstance {
    pub fn validate(&self)->Result<(),String> {
        if !valid_key(&self.function_id)||self.version==0||
            self.overrides.len()>MAX_PARAMETERS {
            return Err("Invalid function reference or version".into());
        }
        let mut keys=HashSet::new();
        for item in &self.overrides {
            if !valid_key(&item.key)||!keys.insert(item.key.as_str()) {
                return Err("Duplicate or invalid material function override key".into());
            }
        }
        Ok(())
    }
}

/// A nested versioned function replaces an unconnected-input Color
/// placeholder in a parent graph. Its child Albedo source becomes the
/// placeholder's outgoing signal when the parent is instantiated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialFunctionCall {
    pub placeholder_node_id:u64,
    pub reference:MaterialFunctionInstance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialFunction {
    pub id:String,
    pub version:u32,
    pub name:String,
    pub graph:Graph,
    /// An intentionally small exposed-parameter API. The first shipped
    /// variant adjusts existing Noise.scale without changing evaluator types.
    #[serde(default)]
    pub parameters:Vec<FunctionParameter>,
    /// Authored-only nested calls; resolved Graph snapshots contain no calls.
    #[serde(default)]
    pub calls:Vec<MaterialFunctionCall>,
}

impl MaterialFunction {
    /// Make the first Noise frequency editable when this template is applied.
    /// More parameters can be exposed by authoring explicit entries.
    pub fn from_graph(id:String,version:u32,graph:Graph)->Result<Self,String> {
        let parameters=graph.nodes.iter()
            .find(|node|matches!(node.kind,NodeKind::Noise{..}))
            .map(|node|vec![FunctionParameter{
                key:"frequency".into(),node_id:node.id,
                kind:FunctionParameterKind::NoiseScale,
            }]).unwrap_or_default();
        let result=Self{name:id.clone(),id,version,graph,parameters,calls:Vec::new()};
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self)->Result<(),String> {
        if !valid_key(&self.id)||self.version==0||
            self.name.trim().is_empty()||self.name.len()>120||
            self.parameters.len()>MAX_PARAMETERS||
            self.calls.len()>MAX_NESTED_CALLS {
            return Err("Invalid material function ID, version, name, or parameter count".into());
        }
        self.graph.validate()?;
        let mut keys=HashSet::new();
        let mut targets=HashSet::new();
        for param in &self.parameters {
            if !valid_key(&param.key) || !keys.insert(param.key.as_str()) ||
                !targets.insert((param.node_id,param.kind as u8)) {
                return Err("Duplicate or invalid material function parameter".into());
            }
            let node=self.graph.node(param.node_id)
                .ok_or("Material function parameter target node does not exist")?;
            match (param.kind,node.kind) {
                (FunctionParameterKind::NoiseScale,NodeKind::Noise{..})=>{},
                (FunctionParameterKind::NestedNoiseScale,NodeKind::Color{..})
                    if self.calls.iter().any(|call|
                        call.placeholder_node_id==param.node_id)=>{},
                _=>return Err("Material function parameter targets wrong node kind".into()),
            }
        }
        let mut placeholders=HashSet::new();
        for call in &self.calls {
            call.reference.validate()?;
            if !placeholders.insert(call.placeholder_node_id) {
                return Err("Duplicate nested function placeholder".into());
            }
            let placeholder=self.graph.node(call.placeholder_node_id)
                .ok_or("Nested function placeholder node does not exist")?;
            if !matches!(placeholder.kind,NodeKind::Color{..}) {
                return Err("Nested function call must replace a Color node".into());
            }
        }
        Ok(())
    }

    pub(crate) fn instantiate(&self,reference:&MaterialFunctionInstance)->Result<Graph,String> {
        self.validate()?;
        reference.validate()?;
        if self.id!=reference.function_id||self.version!=reference.version {
            return Err("Material function reference does not match exact version".into());
        }
        let mut graph=self.graph.clone();
        for item in &reference.overrides {
            let param=self.parameters.iter().find(|param|param.key==item.key)
                .ok_or("Unknown material function override")?;
            match param.kind {
                FunctionParameterKind::NoiseScale | FunctionParameterKind::NestedNoiseScale => {
                    if !(1..=64).contains(&item.value) {
                        return Err("Noise frequency override must be 1..64".into());
                    }
                    if param.kind==FunctionParameterKind::NoiseScale {
                        let node=graph.node_mut(param.node_id)
                            .ok_or("Material function parameter target missing")?;
                        if let NodeKind::Noise{scale,..}=&mut node.kind {
                            *scale=item.value;
                        } else {
                            return Err("Material function parameter target is not Noise".into());
                        }
                    }
                    // NestedNoiseScale is forwarded to the child by the
                    // material_function_nesting resolver, not executed here.
                }
            }
        }
        graph.validate()?;
        Ok(graph)
    }
}

/// Bounded, serializable authoring-time library. Individual material graphs
/// remain self-contained after instantiation; no runtime recursion is possible
/// because a function contains only a Graph of ordinary nodes, never calls.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialFunctionLibrary {
    #[serde(default)]
    pub functions:Vec<MaterialFunction>,
}

impl MaterialFunctionLibrary {
    pub fn validate(&self)->Result<(),String> {
        if self.functions.len()>MAX_FUNCTIONS {
            return Err("Material function library exceeds 128 entries".into());
        }
        let mut versions=HashSet::new();
        for f in &self.functions {
            f.validate()?;
            if !versions.insert((f.id.as_str(),f.version)) {
                return Err("Duplicate material function ID and version".into());
            }
        }
        // These are authoring dependencies; detect missing versions and
        // cycles before a library is considered valid.
        for f in &self.functions {
            self.resolve(&MaterialFunctionInstance{
                function_id:f.id.clone(),version:f.version,overrides:vec![]
            })?;
        }
        Ok(())
    }

    pub fn insert(&mut self,function:MaterialFunction)->Result<usize,String> {
        function.validate()?;
        if self.functions.len()>=MAX_FUNCTIONS {
            return Err("Material function library is full".into());
        }
        if self.functions.iter().any(|item|
            item.id==function.id && item.version==function.version) {
            return Err("That material function ID/version already exists".into());
        }
        self.functions.push(function);
        Ok(self.functions.len()-1)
    }

    pub fn resolve(&self,reference:&MaterialFunctionInstance)->Result<Graph,String> {
        crate::material_function_nesting::resolve(self,reference)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foliage::{FoliageProject,PlantPreset};
    use crate::foliage_materials::MaterialRole;

    fn reference(id:&str,version:u32,scale:u32)->MaterialFunctionInstance {
        MaterialFunctionInstance{function_id:id.into(),version,overrides:vec![
            FunctionOverride{key:"frequency".into(),value:scale}
        ]}
    }

    #[test]
    fn same_template_supports_independent_species_frequency_and_portable_graphs() {
        let mut library=MaterialFunctionLibrary::default();
        let template=MaterialFunction::from_graph(
            "shared_bark".into(),1,Graph::default()).unwrap();
        library.insert(template).unwrap();
        let oak=FoliageProject::from_preset(PlantPreset::Oak);
        let willow=FoliageProject::from_preset(PlantPreset::Willow);
        let mut oak_materials=oak.materials;
        let mut willow_materials=willow.materials;
        oak_materials.apply_function(MaterialRole::Bark,&library,reference("shared_bark",1,4))
            .unwrap();
        willow_materials.apply_function(MaterialRole::Bark,&library,reference("shared_bark",1,19))
            .unwrap();
        let oak_rgba=oak_materials.bake_bark(32,[99,72,50]).unwrap();
        let willow_rgba=willow_materials.bake_bark(32,[99,72,50]).unwrap();
        assert_ne!(oak_rgba,willow_rgba);
        assert_eq!(oak_materials.bark_function.as_ref().unwrap().version,1);
        // Loading a saved tree/asset requires only the resolved graph snapshot.
        let restored:crate::foliage_materials::FoliageMaterials=serde_json::from_str(
            &serde_json::to_string(&oak_materials).unwrap()).unwrap();
        assert_eq!(restored.bake_bark(32,[99,72,50]).unwrap(),oak_rgba);
        assert_eq!(restored.bark_function,oak_materials.bark_function);
    }

    #[test]
    fn existing_references_pin_versions_and_reject_unknown_overrides() {
        let mut library=MaterialFunctionLibrary::default();
        library.insert(MaterialFunction::from_graph(
            "shared_bark".into(),1,Graph::default()).unwrap()).unwrap();
        let mut newer=Graph::default();
        newer.node_mut(1).unwrap().kind=NodeKind::Noise{
            scale:20,octaves:4,seed:42
        };
        library.insert(MaterialFunction::from_graph(
            "shared_bark".into(),2,newer).unwrap()).unwrap();
        let v1=library.resolve(&reference("shared_bark",1,7)).unwrap();
        let v2=library.resolve(&reference("shared_bark",2,7)).unwrap();
        assert_eq!(v1.render_rgba(8,8).unwrap(),v2.render_rgba(8,8).unwrap(),
            "same instance override must produce same result on different templates");
        assert!(library.resolve(&reference("shared_bark",3,7)).is_err());
        let mut malformed=reference("shared_bark",1,7);
        malformed.overrides[0].key="unknown".into();
        assert!(library.resolve(&malformed).is_err());
        malformed.overrides[0].key="frequency".into();
        malformed.overrides.push(malformed.overrides[0].clone());
        assert!(library.resolve(&malformed).is_err());
        malformed.overrides.pop();
        malformed.overrides[0].value=65;
        assert!(library.resolve(&malformed).is_err());
    }

    #[test]
    fn library_json_roundtrip_rejects_duplicate_versions_and_wrong_targets() {
        let function=MaterialFunction::from_graph(
            "bark_noise".into(),1,Graph::default()).unwrap();
        let mut library=MaterialFunctionLibrary::default();
        library.insert(function.clone()).unwrap();
        assert!(library.insert(function.clone()).is_err());
        let decoded:MaterialFunctionLibrary=serde_json::from_str(
            &serde_json::to_string(&library).unwrap()).unwrap();
        decoded.validate().unwrap();
        assert_eq!(library.resolve(&reference("bark_noise",1,8)).unwrap()
                .render_rgba(16,16).unwrap(),
            decoded.resolve(&reference("bark_noise",1,8)).unwrap()
                .render_rgba(16,16).unwrap());
        let mut bad=function;
        bad.parameters[0].node_id=999;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn equivalent_function_instances_share_existing_foliage_bake_cache_entries() {
        use crate::foliage_bake_cache::FoliageBakeCache;
        let mut library=MaterialFunctionLibrary::default();
        library.insert(MaterialFunction::from_graph("oak_bark".into(),1,
            Graph::default()).unwrap()).unwrap();
        library.insert(MaterialFunction::from_graph("willow_bark".into(),1,
            Graph::default()).unwrap()).unwrap();
        let mut oak=crate::foliage_materials::FoliageMaterials::default();
        let mut willow=oak.clone();
        oak.apply_function(MaterialRole::Bark,&library,reference("oak_bark",1,12)).unwrap();
        willow.apply_function(MaterialRole::Bark,&library,
            reference("willow_bark",1,12)).unwrap();
        let mut cache=FoliageBakeCache::new(1024*1024);
        let first=cache.bake_bark(&oak,32,[90,80,70]).unwrap();
        let shared=cache.bake_bark(&willow,32,[90,80,70]).unwrap();
        assert!(std::sync::Arc::ptr_eq(&first,&shared));
        assert_eq!(cache.stats().hits,1);
        willow.apply_function(MaterialRole::Bark,&library,
            reference("willow_bark",1,20)).unwrap();
        let changed=cache.bake_bark(&willow,32,[90,80,70]).unwrap();
        assert_ne!(first.as_ref(),changed.as_ref());
        assert_eq!(cache.stats().misses,2);
    }

    #[test]
    fn missing_template_after_import_does_not_break_existing_asset_snapshot() {
        use crate::asset::VoxelAsset;
        let mut library=MaterialFunctionLibrary::default();
        library.insert(MaterialFunction::from_graph(
            "shared_bark".into(),1,Graph::default()).unwrap()).unwrap();
        let mut asset=VoxelAsset::default();
        asset.materials.apply_function(MaterialRole::Bark,&library,
            reference("shared_bark",1,9)).unwrap();
        let before=asset.materials.bake_bark(32,[90,80,70]).unwrap();
        let text=asset.to_json().unwrap();
        drop(library);
        let after=VoxelAsset::from_json(&text).unwrap();
        assert_eq!(after.materials.bake_bark(32,[90,80,70]).unwrap(),before);
        assert_eq!(after.materials.bark_function,
            asset.materials.bark_function);
    }

    #[test]
    fn assigned_function_is_detached_by_direct_manual_slot_edit() {
        let mut library=MaterialFunctionLibrary::default();
        library.insert(MaterialFunction::from_graph(
            "bark".into(),1,Graph::default()).unwrap()).unwrap();
        let mut materials=crate::foliage_materials::FoliageMaterials::default();
        materials.apply_function(MaterialRole::Bark,&library,reference("bark",1,10))
            .unwrap();
        assert!(materials.bark_function.is_some());
        materials.set_graph(MaterialRole::Bark,Graph::demo_bricks());
        assert!(materials.bark_function.is_none());
    }
}
