//! Versioned, portable publishing contract for the existing material pipeline.
//! FoliageMaterials owns graphs; FoliageBakeCache and GraphRenderCache own
//! bounded reusable bakes. This module only publishes references and fixed
//! appearance snapshots, never defines another material/geometry evaluator.
use crate::asset::VoxelAsset;
use crate::flora_prefab::FloraPrefab;
use crate::foliage::{FoliageProject,LeafGeometry};
use crate::foliage_materials::{FoliageMaterials,SeasonalPalettes};
use crate::foliage_bake_cache::FoliageBakeCache;
use crate::render_cache::GraphRenderCache;
use serde::{Serialize,Deserialize};

pub const RECIPE_VERSION:u32=1;
pub const SNAPSHOT_VERSION:u32=1;
pub const TEXTURE_BAKE_VERSION:u32=1;

/// A stable, non-cryptographic 128-bit *cache identity*, not a security hash.
/// Version and semantic format are part of the hash input. Source image
/// contents are included through Graph's embedded bytes; individual rendered
/// textures are identified separately from their actual RGBA bytes.
fn fingerprint(domain:&str,bytes:&[u8])->String {
    const OFFSET:u128=0x6c62272e07bb014262b821756295c58d;
    const PRIME:u128=0x0000000001000000000000000000013b;
    let mut value=OFFSET;
    for byte in domain.as_bytes().iter().copied()
        .chain(std::iter::once(0u8)).chain(bytes.iter().copied()) {
        value=(value^(byte as u128)).wrapping_mul(PRIME);
    }
    format!("{value:032x}")
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct EmbeddedMaterialSource {
    pub role:String,
    pub name:String,
    pub bitmap_fingerprint:u64,
    /// Import UI currently does not capture an SPDX license or source URI.
    /// Never infer CC0 or clearance from an embedded bitmap filename.
    pub license_status:String,
}
fn embedded_sources(materials:&FoliageMaterials)->Vec<EmbeddedMaterialSource> {
    let mut sources=Vec::new();
    for (role,graph) in [("bark",materials.bark.as_ref()),
                         ("leaf",materials.leaf.as_ref()),
                         ("opacity",materials.leaf_opacity.as_ref())] {
        if let Some(graph)=graph {
            for image in &graph.images {
                sources.push(EmbeddedMaterialSource{
                    role:role.into(),name:image.name.clone(),
                    bitmap_fingerprint:image.fingerprint,
                    license_status:"unverified".into(),
                });
            }
        }
    }
    sources.sort_by(|a,b|(&a.role,&a.name,a.bitmap_fingerprint)
        .cmp(&(&b.role,&b.name,b.bitmap_fingerprint)));
    sources
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct RuntimeMaterialRecipe {
    pub format_version:u32,
    /// Content-derived across Flora individual geometry seeds, Tree and Asset.
    pub recipe_id:String,
    /// Full authoring graphs, function-instance provenance, vector silhouette
    /// and bark meters-per-repeat; no separate source-of-truth graph.
    pub materials:FoliageMaterials,
    pub bark_color:[u8;3],
    pub leaf_color:[u8;3],
    /// Whether there are actual cutout leaf cards to map.
    pub cutout:bool,
    /// Explicit effective palette even for legacy sources with None.
    pub palette:SeasonalPalettes,
    /// Explicit provenance review inventory for user-imported bitmaps.
    /// Native graphs/patterns contain no external images.
    pub embedded_sources:Vec<EmbeddedMaterialSource>,
}
impl RuntimeMaterialRecipe {
    pub fn new(materials:&FoliageMaterials,bark_color:[u8;3],
               leaf_color:[u8;3],cutout:bool)->Result<Self,String> {
        materials.validate()?;
        let palette=materials.effective_palettes(leaf_color);
        let bytes=serde_json::to_vec(&(
            RECIPE_VERSION,materials,bark_color,leaf_color,cutout,&palette,
        )).map_err(|e|e.to_string())?;
        let recipe_id=format!("material/v{RECIPE_VERSION}/{}",
            fingerprint("material-recipe", &bytes));
        Ok(Self{format_version:RECIPE_VERSION,recipe_id,materials:materials.clone(),
            bark_color,leaf_color,cutout,palette,
            embedded_sources:embedded_sources(materials)})
    }
    pub fn from_tree(project:&FoliageProject)->Result<Self,String> {
        project.validate()?;
        Self::new(&project.materials,project.bark_color,project.leaf_color,
            project.leaf_render.geometry==LeafGeometry::CrossedCutout)
    }
    pub fn from_asset(asset:&VoxelAsset)->Result<Self,String> {
        asset.validate()?;
        Self::new(&asset.materials,asset.bark_color,asset.leaf_color,
            asset.leaf_render.geometry==LeafGeometry::CrossedCutout)
    }
    pub fn from_flora(prefab:&FloraPrefab)->Result<Self,String> {
        prefab.validate()?;
        Self::from_tree(&prefab.source)
    }
    pub fn validate(&self)->Result<(),String> {
        if self.format_version!=RECIPE_VERSION {
            return Err("Unsupported material recipe version".into());
        }
        let original=Self::new(&self.materials,self.bark_color,
                               self.leaf_color,self.cutout)?;
        if self.recipe_id!=original.recipe_id || self.palette!=original.palette
            || self.embedded_sources!=original.embedded_sources {
            return Err("Material recipe identity/palette does not match its source".into());
        }
        Ok(())
    }
    pub fn to_json(&self)->Result<String,String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e|e.to_string())
    }
    pub fn from_json(json:&str)->Result<Self,String> {
        let decoded:Self=serde_json::from_str(json).map_err(|e|e.to_string())?;
        decoded.validate()?;
        Ok(decoded)
    }
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct PublishedTexture {
    pub name:String,
    pub color_space:String, // "srgb" or "linear-unorm"
    pub size:[u32;2],
    /// Hash of exact pixels + channel color-space + dimensions and format.
    pub resource_id:String,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct StaticAppearanceSnapshot {
    pub format_version:u32,
    pub recipe_id:String,
    pub snapshot_id:String,
    pub season_progress:f32,
    pub resolution:u32,
    pub textures:Vec<PublishedTexture>,
    pub preview_only_effects:Vec<String>,
}
impl StaticAppearanceSnapshot {
    pub fn from_json(json:&str)->Result<Self,String> {
        let snap:Self=serde_json::from_str(json).map_err(|e|e.to_string())?;
        if snap.format_version!=SNAPSHOT_VERSION ||
           !snap.season_progress.is_finite() ||
           !(0.0..4.0).contains(&snap.season_progress) ||
           !(1..=1024).contains(&snap.resolution) ||
           !snap.recipe_id.starts_with("material/v1/") ||
           !snap.snapshot_id.starts_with("appearance/v1/") ||
           snap.textures.is_empty() ||
           snap.textures.iter().any(|x|
               x.size!=[snap.resolution,snap.resolution] ||
               x.name.is_empty() || x.name.contains('/') || x.name.contains('\\') ||
               !x.resource_id.starts_with("texture/v1/") ||
               (x.color_space!="srgb" && x.color_space!="linear-unorm")) ||
           {
               let mut files=std::collections::HashSet::new();
               snap.textures.iter().any(|x|!files.insert(&x.name))
           } {
            return Err("Invalid static appearance snapshot".into());
        }
        Ok(snap)
    }
}

pub struct MaterialPublication {
    pub recipe:RuntimeMaterialRecipe,
    pub snapshot:StaticAppearanceSnapshot,
    /// PNG-ready RGBA8 images, owned by the export operation.
    pub pixels:Vec<(String,Vec<u8>)>,
}

fn texture_color_space(name:&str)->&'static str {
    if name.contains("-roughness") || name.contains("-metallic") ||
        name.contains("-normal") || name.contains("-alpha") {
        "linear-unorm"
    } else {"srgb"}
}

/// Exact per-texel *legacy seasonal shader* coloration (without the
/// preview's separate dynamic flower masks, wind, GGX or IBL).
pub fn bake_leaf_season(base:&[u8],size:u32,leaf_tint:[u8;3],
                        palette:&SeasonalPalettes,season:f32)->Result<Vec<u8>,String> {
    if size==0 || size>1024 || base.len()!=size as usize*size as usize*4 ||
       !season.is_finite() || !(0.0..=4.0).contains(&season) {
        return Err("Invalid leaf snapshot size, color bytes or season".into());
    }
    let mut result=base.to_vec();
    for (index,px) in result.chunks_exact_mut(4).enumerate() {
        let y=(index as u32/size) as f32;
        let v=(y+0.5)/size as f32;
        let seasonal=palette.sample(season,1.0-v);
        for channel in 0..3 {
            let base=(leaf_tint[channel] as f32/255.0).max(0.12);
            let color=(px[channel] as f32/255.0*seasonal[channel]/base)
                .min(1.0);
            px[channel]=(color*255.0).round().clamp(0.0,255.0) as u8;
        }
        // Keep original silhouette, subpixel coverage and fruit alpha.
    }
    Ok(result)
}

/// Publish one selected season, sharing byte-identical textures across
/// Tree, Asset, and differently seeded Flora instances through the
/// already existing two bounded bake caches.
pub fn publish(recipe:&RuntimeMaterialRecipe,size:u32,season:f32,
               foliage_cache:&mut FoliageBakeCache,
               graph_cache:&mut GraphRenderCache)->Result<MaterialPublication,String> {
    recipe.validate()?;
    if size==0 || size>1024 || !season.is_finite()
        || !(0.0..=4.0).contains(&season) {
        return Err("Publication size must be 1..1024 and season 0..4".into());
    }
    let mut pixels=Vec::new();
    let mats=&recipe.materials;
    if mats.bark.is_some() {
        pixels.push(("bark-albedo.png".into(),
            foliage_cache.bake_bark(mats,size,recipe.bark_color)?.to_vec()));
    }
    if recipe.cutout {
        let leaf=foliage_cache.bake_leaf(mats,size,recipe.leaf_color)?;
        pixels.push(("foliage-leaves.png".into(),leaf.to_vec()));
        pixels.push(("foliage-leaves-alpha.png".into(),
            leaf.chunks_exact(4).flat_map(|px|[px[3],px[3],px[3],255]).collect()));
        pixels.push(("foliage-leaves-season.png".into(),
            bake_leaf_season(&leaf,size,recipe.leaf_color,&recipe.palette,season)?));
    }
    for (name,graph) in [("bark",mats.bark.as_ref()),
                         ("foliage-leaves",mats.leaf.as_ref())] {
        let Some(graph)=graph else {continue;};
        pixels.push((format!("{name}-roughness.png"),
            graph_cache.render_channel(graph,1,size,size)?.to_vec()));
        if name=="bark" {
            pixels.push(("bark-metallic.png".into(),
                graph_cache.render_channel(graph,2,size,size)?.to_vec()));
        }
        let normal=graph_cache.render_normal(graph,size,size)?;
        pixels.push((format!("{name}-normal.png"),normal.to_vec()));
        let mut flipped=normal.to_vec();
        for pixel in flipped.chunks_exact_mut(4) {pixel[1]=255-pixel[1];}
        pixels.push((format!("{name}-normal-obj.png"),flipped));
    }
    let textures=pixels.iter().map(|(name,pixels)|{
        let color_space=texture_color_space(name);
        PublishedTexture{name:name.clone(),color_space:color_space.into(),
            size:[size,size],
            resource_id:format!("texture/v{TEXTURE_BAKE_VERSION}/{}",
                fingerprint(&format!("{color_space}/rgba8/{size}x{size}"),pixels)),
        }
    }).collect::<Vec<_>>();
    let normalized_season=season.rem_euclid(4.0);
    let snapshot_key=serde_json::to_vec(&(
        SNAPSHOT_VERSION,&recipe.recipe_id,size,normalized_season.to_bits(),
        textures.iter().map(|t|(&t.name,&t.resource_id)).collect::<Vec<_>>(),
    )).map_err(|e|e.to_string())?;
    let snapshot=StaticAppearanceSnapshot {
        format_version:SNAPSHOT_VERSION,
        recipe_id:recipe.recipe_id.clone(),
        snapshot_id:format!("appearance/v{SNAPSHOT_VERSION}/{}",
            fingerprint("static-appearance",&snapshot_key)),
        season_progress:normalized_season,
        resolution:size,
        textures,
        preview_only_effects:vec![
            "wind animation".into(),
            "spring blossom/flower overlays".into(),
            "GPU GGX, IBL and leaf transmission lighting".into(),
            "ordered/MSAA alpha coverage".into(),
            "two-sample bark seam shader blend".into(),
        ],
    };
    Ok(MaterialPublication{recipe:recipe.clone(),snapshot,pixels})
}

impl MaterialPublication {
    pub fn pixel(&self,name:&str)->Option<&[u8]> {
        self.pixels.iter().find(|(n,_)|n==name)
            .map(|(_,v)|v.as_slice())
    }
    pub fn static_mtl(&self,extended_mtl:&str)->String {
        extended_mtl.replace("map_Kd foliage-leaves.png",
                              "map_Kd foliage-leaves-season.png")
    }
}
