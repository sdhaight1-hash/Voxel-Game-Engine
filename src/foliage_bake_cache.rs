//! Bounded reuse of final Bark and Leaf RGBA bakes between Tree Designer and
//! editable Asset previews. Generation, occupancy and GPU resources are not
//! owned by this cache; FoliageMaterials remains the source of appearance.
use crate::foliage_materials::FoliageMaterials;
use std::{collections::VecDeque, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role { Bark, Leaf }

/// Only fields read by the requested bake belong to the recipe. In particular
/// season palettes are dynamic GPU data and never invalidate a leaf bake.
#[derive(PartialEq, Eq)]
struct BakeKey {
    role: Role,
    size: u32,
    tint: [u8; 3],
    recipe: Vec<u8>,
}

struct BakeEntry {
    key: BakeKey,
    rgba: Arc<[u8]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BakeCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
    pub resident_bytes: usize,
    pub peak_resident_bytes: usize,
}

pub struct FoliageBakeCache {
    limit_bytes: usize,
    entries: VecDeque<BakeEntry>,
    stats: BakeCacheStats,
}

impl FoliageBakeCache {
    pub fn new(limit_bytes: usize) -> Self {
        Self { limit_bytes, entries: VecDeque::new(),
            stats: BakeCacheStats::default() }
    }

    pub fn stats(&self) -> BakeCacheStats { self.stats }

    pub fn bake_leaf(&mut self,materials:&FoliageMaterials,
                     size:u32,tint:[u8;3])->Result<Arc<[u8]>,String> {
        self.bake(materials,size,tint,Role::Leaf)
    }

    pub fn bake_bark(&mut self,materials:&FoliageMaterials,
                     size:u32,tint:[u8;3])->Result<Arc<[u8]>,String> {
        self.bake(materials,size,tint,Role::Bark)
    }

    fn bake(&mut self,materials:&FoliageMaterials,size:u32,tint:[u8;3],role:Role)
        ->Result<Arc<[u8]>,String> {
        // Always validate before cache lookup, even for recipes seen earlier.
        materials.validate()?;
        if size==0 || size>1024 {
            return Err("Foliage bake must be 1..1024 px".into());
        }
        let recipe=match role {
            Role::Leaf => serde_json::to_vec(&(
                &materials.leaf, &materials.leaf_opacity, materials.leaf_species,
                materials.fruit_color, materials.cutout_threshold,
            )),
            Role::Bark => serde_json::to_vec(&(
                &materials.bark, materials.bark_tint_strength,
            )),
        }.map_err(|err|err.to_string())?;
        let key=BakeKey{role,size,tint,recipe};
        if let Some(index)=self.entries.iter().position(|entry|entry.key==key) {
            let entry=self.entries.remove(index).expect("indexed cache entry");
            let pixels=entry.rgba.clone();
            self.entries.push_front(entry);
            self.stats.hits+=1;
            return Ok(pixels);
        }

        self.stats.misses+=1;
        let rgba=match role {
            Role::Leaf=>materials.bake_leaf(size,tint)?,
            Role::Bark=>materials.bake_bark(size,tint)?,
        };
        let pixels:Arc<[u8]>=rgba.into();
        if pixels.len()<=self.limit_bytes {
            while self.stats.resident_bytes>self.limit_bytes-pixels.len() {
                let old=self.entries.pop_back().expect("over limit requires an entry");
                self.stats.resident_bytes-=old.rgba.len();
                self.stats.evictions+=1;
            }
            self.stats.resident_bytes+=pixels.len();
            self.stats.peak_resident_bytes=self.stats.peak_resident_bytes
                .max(self.stats.resident_bytes);
            self.entries.push_front(BakeEntry{key,rgba:pixels.clone()});
            self.stats.entries=self.entries.len();
        }
        Ok(pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Graph, NodeKind};
    use crate::foliage::PlantPreset;
    use crate::foliage_materials::SeasonalPalettes;

    #[test]
    fn identical_species_bakes_share_arc_and_preserve_exact_pixels() {
        let first=FoliageMaterials::for_species(PlantPreset::Willow);
        let second=first.clone();
        let mut cache=FoliageBakeCache::new(8*1024*1024);
        let a=cache.bake_leaf(&first,64,[80,150,70]).unwrap();
        let b=cache.bake_leaf(&second,64,[80,150,70]).unwrap();
        assert!(Arc::ptr_eq(&a,&b));
        assert_eq!(a.as_ref(),first.bake_leaf(64,[80,150,70]).unwrap());
        let bark=cache.bake_bark(&first,64,[99,76,55]).unwrap();
        let second_bark=cache.bake_bark(&second,64,[99,76,55]).unwrap();
        assert!(Arc::ptr_eq(&bark,&second_bark));
        assert_eq!(bark.as_ref(),first.bake_bark(64,[99,76,55]).unwrap());
        assert_eq!(cache.stats().hits,2);
        assert_eq!(cache.stats().misses,2);
    }

    #[test]
    fn seasonal_uniforms_and_unrelated_graph_slots_do_not_rebake() {
        let base=FoliageMaterials::for_species(PlantPreset::Oak);
        let mut cache=FoliageBakeCache::new(1024*1024);
        let leaf=cache.bake_leaf(&base,32,[65,122,70]).unwrap();
        let bark=cache.bake_bark(&base,32,[104,81,57]).unwrap();
        let mut edited=base.clone();
        let mut palette=SeasonalPalettes::default();
        palette.seasons[2][0]=[199,45,27];
        edited.seasonal_palettes=Some(palette);
        assert!(Arc::ptr_eq(&leaf,
            &cache.bake_leaf(&edited,32,[65,122,70]).unwrap()));
        assert!(Arc::ptr_eq(&bark,
            &cache.bake_bark(&edited,32,[104,81,57]).unwrap()));
        edited.bark_tint_strength=0.7;
        assert!(Arc::ptr_eq(&leaf,
            &cache.bake_leaf(&edited,32,[65,122,70]).unwrap()));
        assert!(!Arc::ptr_eq(&bark,
            &cache.bake_bark(&edited,32,[104,81,57]).unwrap()));
        edited.leaf=Some(Graph::default());
        assert!(!Arc::ptr_eq(&leaf,
            &cache.bake_leaf(&edited,32,[65,122,70]).unwrap()));
    }

    #[test]
    fn tint_size_and_opacity_change_only_affected_recipe() {
        let mut mats=FoliageMaterials::default();
        mats.leaf=Some(Graph::default());
        let mut cache=FoliageBakeCache::new(1024*1024);
        let baseline=cache.bake_leaf(&mats,16,[71,141,90]).unwrap();
        assert!(!Arc::ptr_eq(&baseline,
            &cache.bake_leaf(&mats,16,[101,141,90]).unwrap()));
        assert!(!Arc::ptr_eq(&baseline,
            &cache.bake_leaf(&mats,32,[71,141,90]).unwrap()));
        mats.leaf_opacity=Some(Graph::default());
        assert!(!Arc::ptr_eq(&baseline,
            &cache.bake_leaf(&mats,16,[71,141,90]).unwrap()));
        mats.leaf.as_mut().unwrap().nodes[0].kind=NodeKind::Checker{tiles:8};
        assert_eq!(cache.bake_leaf(&mats,16,[71,141,90]).unwrap().len(),
            16*16*4);
    }

    #[test]
    fn bounded_lru_and_oversize_bypass_do_not_change_bytes() {
        let mats=FoliageMaterials::default();
        let mut cache=FoliageBakeCache::new(8*8*4);
        let first=cache.bake_bark(&mats,8,[1,2,3]).unwrap();
        cache.bake_bark(&mats,8,[4,5,6]).unwrap();
        assert_eq!(cache.stats().resident_bytes,256);
        assert_eq!(cache.stats().evictions,1);
        assert_eq!(first.as_ref(),mats.bake_bark(8,[1,2,3]).unwrap());
        cache.bake_bark(&mats,16,[7,8,9]).unwrap();
        assert_eq!(cache.stats().entries,1);
        assert!(cache.stats().peak_resident_bytes<=256);
        assert_eq!(cache.stats().misses,3);
    }

    #[test]
    fn invalid_inputs_never_return_stale_cached_pixels() {
        let mats=FoliageMaterials::default();
        let mut cache=FoliageBakeCache::new(4096);
        cache.bake_bark(&mats,8,[20,30,40]).unwrap();
        let mut invalid=mats;
        invalid.bark_tint_strength=f32::NAN;
        assert!(cache.bake_bark(&invalid,8,[20,30,40]).is_err());
        assert!(cache.bake_leaf(&invalid,8,[20,30,40]).is_err());
        assert!(cache.bake_bark(&FoliageMaterials::default(),0,[20,30,40]).is_err());
        assert_eq!(cache.stats().hits,0);
    }
}
