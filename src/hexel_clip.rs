//! Convex clipping for the game's regular, one-metre hexagonal prisms.
//! Mesh pieces are owned by actual occupied hexels; this is only render data.
//! Local authored trees use Z-up, while a future world import uses Y-up.

const SQRT3: f32 = 1.732_050_8;
const EPS: f32 = 0.000_002;

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

pub type ClippedTriangle = [Vertex; 3];

fn lerp(a: Vertex, b: Vertex, t: f32) -> Vertex {
    Vertex {
        position: std::array::from_fn(|i|
            a.position[i] + (b.position[i]-a.position[i])*t),
        uv: std::array::from_fn(|i| a.uv[i]+(b.uv[i]-a.uv[i])*t),
    }
}

pub fn hex_center(cell: [i32;3], size: f32) -> [f32;3] {
    [
        (cell[0] as f32+0.5*cell[1] as f32)*size,
        cell[1] as f32*(SQRT3*0.5)*size,
        (cell[2] as f32+0.5)*size,
    ]
}

/// Fixed side normals for the six faces of a pointy-vertex hexagon.
/// Avoid trigonometric functions in the hot per-triangle clipping loop.
const NORMALS: [[f32;2];6]=[
    [1.0,0.0],[0.5,0.866_025_4],[-0.5,0.866_025_4],
    [-1.0,0.0],[-0.5,-0.866_025_4],[0.5,-0.866_025_4],
];

fn signed_distance(pos:[f32;3],c:[f32;3],size:f32,plane:usize)->f32 {
    match plane {
        0..=5=>{
            let n=NORMALS[plane];
            n[0]*(pos[0]-c[0])+n[1]*(pos[1]-c[1])-0.5*size
        }
        6=>pos[2]-c[2]-0.5*size,
        _=>c[2]-pos[2]-0.5*size,
    }
}

pub fn contains(pos:[f32;3],cell:[i32;3],size:f32)->bool {
    let center=hex_center(cell,size);
    (0..8).all(|i|signed_distance(pos,center,size,i)<=0.000_03)
}

pub fn clip_to_hex_prism(triangle: ClippedTriangle,cell:[i32;3],size:f32)
    ->Vec<ClippedTriangle> {
    let center=hex_center(cell,size);
    // Reject whole triangles against any plane without allocating.
    if (0..8).any(|plane|triangle.iter().all(|v|
        signed_distance(v.position,center,size,plane)>EPS)) {
        return Vec::new();
    }
    // Convex region fast path; the full triangle is already inside.
    if triangle.iter().all(|v|contains(v.position,cell,size)) {
        return vec![triangle];
    }
    let mut polygon=triangle.to_vec();
    for plane in 0..8 {
        if polygon.is_empty(){break;}
        let mut next=Vec::with_capacity(polygon.len()+1);
        let mut previous=*polygon.last().unwrap();
        let mut previous_d=signed_distance(previous.position,center,size,plane);
        for &current in &polygon {
            let d=signed_distance(current.position,center,size,plane);
            let inside=d<=EPS;
            let was_inside=previous_d<=EPS;
            if inside!=was_inside {
                let denom=previous_d-d;
                let t=if denom.abs()<f32::EPSILON {0.5} else {
                    (previous_d/denom).clamp(0.0,1.0)
                };
                next.push(lerp(previous,current,t));
            }
            if inside {next.push(current);}
            previous=current;
            previous_d=d;
        }
        polygon=next;
    }
    if polygon.len()<3 {return Vec::new();}
    let mut result=Vec::new();
    for i in 1..polygon.len()-1 {
        let t=[polygon[0],polygon[i],polygon[i+1]];
        let a=t[0].position;
        let b=t[1].position;
        let c=t[2].position;
        let u=[b[0]-a[0],b[1]-a[1],b[2]-a[2]];
        let v=[c[0]-a[0],c[1]-a[1],c[2]-a[2]];
        let cross=[u[1]*v[2]-u[2]*v[1],
                   u[2]*v[0]-u[0]*v[2],
                   u[0]*v[1]-u[1]*v[0]];
        if cross.iter().map(|x|x*x).sum::<f32>()>1e-12 {
            result.push(t);
        }
    }
    result
}

/// Small conservative list of cells that can intersect one render triangle.
/// Axial q=x/s-y/(sqrt(3)*s), r=2y/(sqrt(3)*s).
/// Candidate rounding is deliberately padded, then exact clipping decides.
pub fn triangle_candidates(triangle: ClippedTriangle,size:f32)
    ->impl Iterator<Item=[i32;3]> {
    let mut min=[f32::INFINITY;3];
    let mut max=[f32::NEG_INFINITY;3];
    for vertex in triangle {
        let [x,y,z]=vertex.position;
        let r=2.0*y/(SQRT3*size);
        let coordinates=[x/size-0.5*r,r,z/size];
        for k in 0..3 {
            min[k]=min[k].min(coordinates[k]);
            max[k]=max[k].max(coordinates[k]);
        }
    }
    let lower=[(min[0].floor() as i32)-1,(min[1].floor() as i32)-1,
        (min[2].floor() as i32)-1];
    let upper=[(max[0].ceil() as i32)+1,(max[1].ceil() as i32)+1,
        (max[2].ceil() as i32)+1];
    (lower[0]..=upper[0]).flat_map(move |q| {
        (lower[1]..=upper[1]).flat_map(move |r| {
            (lower[2]..=upper[2]).map(move |z|[q,r,z])
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(p:[f32;3])->Vertex {Vertex{position:p,uv:[p[0],p[1]]}}

    #[test]
    fn hex_prisms_have_correct_shared_faces_and_vertical_layers() {
        assert!(contains([0.0,0.0,0.5],[0,0,0],1.0));
        assert!(contains([0.5,0.0,0.5],[0,0,0],1.0));
        assert!(contains([0.5,0.0,0.5],[1,0,0],1.0));
        assert!(!contains([0.5002,0.0,0.5],[0,0,0],1.0));
        assert!(!contains([0.0,0.0,1.2],[0,0,0],1.0));
        assert!(contains([0.0,0.0,1.2],[0,0,1],1.0));
        for q in -4..=4 {
            for r in -4..=4 {
                let pos=hex_center([q,r,2],1.0);
                assert!(contains(pos,[q,r,2],1.0));
            }
        }
    }

    #[test]
    fn clip_keeps_positions_and_interpolated_uvs_inside_one_hexel() {
        let triangle=[v([-0.8,0.0,0.25]),v([0.8,0.0,0.75]),
            v([0.0,0.9,1.25])];
        let pieces=clip_to_hex_prism(triangle,[0,0,0],1.0);
        assert!(!pieces.is_empty());
        for tri in pieces {
            for p in tri {
                assert!(contains(p.position,[0,0,0],1.0),"{p:?}");
                assert!((p.uv[0]-p.position[0]).abs()<1e-5);
                assert!((p.uv[1]-p.position[1]).abs()<1e-5);
            }
        }
    }

    #[test]
    fn clipping_crosses_adjacent_hexels_without_missing_owner() {
        let triangle=[v([0.25,-0.1,0.2]),v([0.75,-0.1,0.2]),
            v([0.5,0.2,0.8])];
        let a=clip_to_hex_prism(triangle,[0,0,0],1.0);
        let b=clip_to_hex_prism(triangle,[1,0,0],1.0);
        assert!(!a.is_empty()&&!b.is_empty());
        let candidates=triangle_candidates(triangle,1.0).collect::<Vec<_>>();
        assert!(candidates.contains(&[0,0,0]));
        assert!(candidates.contains(&[1,0,0]));
        assert!(clip_to_hex_prism(triangle,[8,0,0],1.0).is_empty());
    }
}
