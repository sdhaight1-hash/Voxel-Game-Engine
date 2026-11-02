# Vegetation climate profiles — 43 built-in foliage presets

**Status:** Implemented in `src/plant_climate.rs` as *working, approximate placement preferences*. Not 43 universal biological survival limits. Intended for the planned synthetic toroidal world climate/biome maps, **not** connected to a world generator yet.

**Measurements and interpretation**
- **Mean T (°C):** preferred *mean annual air temperature*. Not the same as an absolute hottest/coldest day or growth-season temperature.
- **Rain (mm/y):** preferred annual atmospheric precipitation, before irrigation, groundwater, snowmelt or fog drip.
- **Cold event (°C):** approximate minimum of a brief annual cold episode for game-placement filtering; actual survival depends on duration, maturity, cultivar, dormancy and microclimate. **Not a measured universal lethal threshold.**
- **Dry months:** preferred count of months with <30 mm of precipitation; rough seasonality proxy, not drought physiology.
- **Root-zone soil:** broad site moisture `Dry`, `Moderate`, `Moist`, or `Wet`. Site water availability is distinct from regional rainfall.
- **Light:** `Sun` or `PartialShade` (understory shade available as a third state).
- **Groundwater:** required *reliable root-zone access to stream, alluvial groundwater or irrigation*, not proximity to arbitrary ocean saltwater. This prevents cottonwood forest from spontaneously spawning on dry desert slopes and dates from spawning in waterless sand.

`PlantPreset::climate_profile()` supplies every built-in profile. `ClimateProfile::suitability(SiteClimate)` calculates a deterministic 0–1 placement weight from climate fields and water access. `ClimateCatalog::built_in().to_pretty_json()` produces versioned, portable JSON (`hexel-vegetation-climate-v1`) for a later world-runtime handoff. **Neither function currently runs world placement.** The temperature, rain and dry-month preferences use softened edges; cold-event floor and required groundwater are zero-weight gates.

## Complete profile table

| # | Plant | Mean T °C | Rain mm/y | Cold event °C | Dry months | Soil | Light | Groundwater |
|---:|---|---:|---:|---:|---:|---|---|---|
| 1 | Acacia | 19–30 | 250–1100 | -3 | 3–7 | Dry | Sun | No |
| 2 | Birch | 0–12 | 450–1500 | -40 | 0–4 | Moist | Sun | No |
| 3 | Ebony | 22–29 | 1200–3000 | 8 | 0–3 | Moist | Partial shade | No |
| 4 | Kapok | 23–29 | 1200–3000 | 10 | 1–4 | Moist | Sun | No |
| 5 | Maple | 7–17 | 650–1600 | -30 | 0–3 | Moist | Partial shade | No |
| 6 | Crimson King Maple | 7–17 | 650–1600 | -30 | 0–3 | Moist | Sun | No |
| 7 | Oak | 7–19 | 500–1400 | -25 | 1–4 | Moderate | Sun | No |
| 8 | Purpleheart | 24–29 | 1700–3500 | 13 | 0–2 | Wet | Partial shade | No |
| 9 | Walnut | 8–17 | 500–1400 | -28 | 1–3 | Moderate | Sun | No |
| 10 | Willow | 5–20 | 600–1800 | -30 | 0–4 | Wet | Sun | Required |
| 11 | Cherry Blossom | 9–17 | 750–1700 | -20 | 0–2 | Moist | Sun | No |
| 12 | Kentucky Coffeetree | 8–18 | 600–1300 | -32 | 1–3 | Moderate | Sun | No |
| 13 | Eastern Cottonwood | 5–19 | 300–1500 | -38 | 0–5 | Moist | Sun | Required |
| 14 | Fremont Cottonwood | 12–24 | 100–800 | -18 | 2–7 | Moist | Sun | Required |
| 15 | Black Cottonwood | 4–17 | 200–2500 | -32 | 0–4 | Moist | Sun | Required |
| 16 | Bald Cypress | 13–24 | 900–1800 | -25 | 0–4 | Wet | Sun | No |
| 17 | Mediterranean Cypress | 12–23 | 300–1000 | -18 | 2–6 | Dry | Sun | No |
| 18 | Larch | -2–12 | 400–1500 | -45 | 0–4 | Moderate | Sun | No |
| 19 | Pine | 1–20 | 350–1700 | -35 | 1–5 | Moderate | Sun | No |
| 20 | Redwood | 10–17 | 850–2600 | -10 | 1–4 | Moist | Partial shade | No |
| 21 | Red Apple | 6–15 | 600–1350 | -30 | 1–3 | Moderate | Sun | No |
| 22 | Pink Apple | 6–15 | 600–1350 | -30 | 1–3 | Moderate | Sun | No |
| 23 | Yellow Apple | 6–15 | 600–1350 | -30 | 1–3 | Moderate | Sun | No |
| 24 | Pear | 7–17 | 600–1300 | -27 | 1–3 | Moderate | Sun | No |
| 25 | Olive | 13–23 | 300–900 | -8 | 3–6 | Dry | Sun | No |
| 26 | Peach | 10–19 | 500–1400 | -22 | 1–4 | Moderate | Sun | No |
| 27 | Cherry | 7–16 | 600–1300 | -28 | 1–3 | Moderate | Sun | No |
| 28 | Mango | 22–30 | 750–2400 | 5 | 2–5 | Moderate | Sun | No |
| 29 | Orange | 16–27 | 700–1800 | -4 | 0–3 | Moderate | Sun | No |
| 30 | Lychee | 18–28 | 1000–2200 | 0 | 0–3 | Moist | Sun | No |
| 31 | Breadfruit | 23–31 | 1500–3000 | 10 | 0–2 | Moist | Sun | No |
| 32 | Pomegranate | 14–28 | 300–1000 | -12 | 3–7 | Dry | Sun | No |
| 33 | Arabica Coffee | 18–23 | 1200–2200 | 2 | 1–3 | Moist | Partial shade | No |
| 34 | Caturra Coffee | 18–23 | 1200–2200 | 2 | 1–3 | Moist | Partial shade | No |
| 35 | Maragogipe Coffee | 18–23 | 1200–2200 | 2 | 1–3 | Moist | Partial shade | No |
| 36 | Castillo Coffee | 18–24 | 1200–2200 | 2 | 1–3 | Moist | Partial shade | No |
| 37 | Tabi Coffee | 17–23 | 1200–2200 | 2 | 1–3 | Moist | Partial shade | No |
| 38 | Green Bamboo | 12–27 | 900–2300 | -10 | 0–4 | Moist | Partial shade | No |
| 39 | Brown Bamboo | 12–27 | 900–2300 | -10 | 0–4 | Moist | Partial shade | No |
| 40 | Coconut Palm | 24–30 | 1200–2400 | 10 | 0–3 | Moist | Sun | No |
| 41 | Date Palm | 19–31 | 100–400 | -8 | 6–10 | Moderate | Sun | Required |
| 42 | Fan Palm | 14–28 | 300–1300 | -8 | 2–5 | Moderate | Sun | No |
| 43 | Shrub | 5–27 | 300–2000 | -25 | 0–6 | Moderate | Partial shade | No |

## Taxonomic and ecological assumptions

- **Acacia:** Savanna-type acacia approximation; genus spans many climates.
- **Birch:** Cool temperate/boreal Betula.
- **Ebony:** Tropical Diospyros-type approximation.
- **Kapok:** Tropical Ceiba pentandra.
- **Maple:** Temperate Acer approximation.
- **Crimson King Maple:** Norway maple ornamental cultivar.
- **Oak:** Temperate Quercus approximation.
- **Purpleheart:** Humid tropical Peltogyne.
- **Walnut:** Temperate Juglans approximation.
- **Willow:** Riparian Salix; water access is critical.
- **Cherry Blossom:** Ornamental flowering Prunus.
- **Kentucky Coffeetree:** Gymnocladus; often streamside but tolerates urban sites.
- **Eastern Cottonwood:** Populus deltoides riparian tree.
- **Fremont Cottonwood:** Southwestern riparian Populus; river access vital in deserts.
- **Black Cottonwood:** Populus trichocarpa; wet alluvial sites.
- **Bald Cypress:** Taxodium; wetlands and well-watered sites.
- **Mediterranean Cypress:** Cupressus sempervirens Mediterranean type.
- **Larch:** Cold tolerant Larix approximation.
- **Pine:** Temperate Pinus approximation.
- **Redwood:** Coastal Sequoia; fog effect not directly encoded.
- **Red Apple:** Temperate Malus; winter chill not modeled.
- **Pink Apple:** Same Malus environmental envelope; color variant.
- **Yellow Apple:** Same Malus environmental envelope; color variant.
- **Pear:** Temperate Pyrus; winter chill not modeled.
- **Olive:** Mediterranean Olea europaea.
- **Peach:** Temperate Prunus persica; winter chill not modeled.
- **Cherry:** Temperate fruiting Prunus; winter chill not modeled.
- **Mango:** Tropical Mangifera indica.
- **Orange:** Sweet orange Citrus sinensis.
- **Lychee:** Warm humid Litchi chinensis.
- **Breadfruit:** Tropical Artocarpus altilis.
- **Pomegranate:** Mediterranean/warm dry Punica granatum.
- **Arabica Coffee:** Coffea arabica crop; shade-tolerant.
- **Caturra Coffee:** Coffea arabica dwarf Bourbon mutation; shared ecology.
- **Maragogipe Coffee:** Large-fruited Typica mutation; shared ecology.
- **Castillo Coffee:** Colombian coffee cultivar; shared Arabica climate.
- **Tabi Coffee:** Colombian coffee cultivar; shared Arabica climate.
- **Green Bamboo:** Generic bamboo type, not a specified species.
- **Brown Bamboo:** Bamboo color variant, not a specified species.
- **Coconut Palm:** Cocos nucifera maritime tropics.
- **Date Palm:** Phoenix dactylifera desert oases; groundwater or irrigation.
- **Fan Palm:** Warm-climate Washingtonia-like fan palm approximation.
- **Shrub:** Non-taxonomic generic shrub; intentionally broad.

A name such as `Acacia`, `Ebony`, `Oak`, `Pine`, `Maple`, `Bamboo`, `Fan Palm`, or `Shrub` is not a pinned botanical species. The corresponding envelope represents a **stated game-art archetype**, not the whole genus or every species with that name. Apple color variants share the same climate data, as do unspecified bamboo color variants. Coffee cultivars remain variants of `Coffea arabica`; local farming origin does not itself define a different climate niche.

## Research anchors and important caveats

- [FAO ECOCROP catalog](https://www.fao.org/geospatial/data-and-tools/data-portals/ecocrop/) — source framework for ecological temperature/rainfall/soil constraints; some tables provide both *optimal* and *absolute* ranges. These implemented numbers are manually selected game-placement values, **not row-by-row direct imports from Ecocrop**.
- [FAO Coffea arabica](https://ecocrop.apps.fao.org/ecocrop/srv/en/dataSheet?id=749) — broad optimal 14–28 °C, 1,400–2,300 mm precipitation; [coffee agronomy synthesis](https://www.coffearium.org/entries/site-selection-and-climate/) supports narrower 18–21 °C and 1,200–1,800 mm preferences. We use a broad playable compromise of 18–23 °C and 1,200–2,200 mm. Coffee's actual flowering may follow *dry spell then rain*, not a fixed spring date, and frost sensitivity is complex.
- [FAO Coconut](https://ecocrop.apps.fao.org/ecocrop/srv/en/dataSheet?id=744), [FAO Date Palm](https://ecocrop.apps.fao.org/ecocrop/srv/en/dataSheet?id=1673), [FAO Breadfruit](https://ecocrop.apps.fao.org/ecocrop/srv/en/dataSheet?id=3423) — tropical vs desert-oasis contrasts. Date-palm *thermal growing-season optimum* should not be misread as an annual-mean temperature.
- [USFS Eastern Cottonwood](https://research.fs.usda.gov/feis/species-reviews/popdel), [Fremont Cottonwood](https://research.fs.usda.gov/feis/species-reviews/popfre), [Black Cottonwood](https://research.fs.usda.gov/feis/species-reviews/popbalt) — wide rainfall envelopes but riparian groundwater often decisive.
- [USFS Coast Redwood](https://research.fs.usda.gov/silvics/redwood) — seasonal fog significantly compensates for summer drought. **Fog is not represented** as a separate input yet, so coastal distribution cannot be fully reconstructed by rainfall alone.
- [USDA Kentucky Coffeetree](https://www.nrcs.usda.gov/sites/default/files/2023-09/tree_booklet_2023_low_Res.pdf) — moist streamside native habitat but adaptable to various soils as an established landscape tree.
- [University of California fruit-tree chill requirements](https://www.ucanr.edu/site/uc-marin-master-gardeners/article/january-2022-choosing-bare-root-fruit-trees) — apples, pears, cherries and peaches need enough winter chill to flower and fruit reliably. **Chill hours are not implemented**, so habitat *growth* suitability here is not equivalent to crop yield suitability.
- [World Agroforestry tree database](https://apps.worldagroforestry.org/treedb/) — wider tree habitat reference.

## Next world-runtime interfaces, not implemented

1. For each geographic tile/planting cell, derive annual mean temperature and seasonal rainfall from saved **periodic** climate maps. Cold-event estimates should come from variability, not mean alone.
2. Resolve `groundwater_access` and root-zone moisture from *finished* rivers, floodplains, lakes, water table and irrigation. Do not treat rainfall alone as water availability.
3. Evaluate the authored species' climate score and combine it with local elevation, slope, soil/drainage, competition, forest density and a stable RNG seed. Do not force exclusive, hard-coded biomes to replace geographic fields.
4. Add fog/coastal humidity, crop chill hours, soil salinity/pH, drought evapotranspiration, floods, disturbance and growth/life-cycle simulation as separate revisions if needed. Current score is for **initial placement**, not dynamic growth, health, yield or migration.
5. Version and pin exported profile sets within world saves. Changing climate weights after a world is built must not silently regenerate placed trees under player modifications.

