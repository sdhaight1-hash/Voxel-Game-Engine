//! Bounded editable scalar curve with monotone segment-preserving cubic
//! Hermite interpolation. This modifies authored material values only.
use serde::{Deserialize,Serialize};

pub const MAX_CURVE_POINTS:usize=8;

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize)]
pub struct CurvePoint {
    pub position:f32,
    pub value:f32,
}

const EMPTY:CurvePoint=CurvePoint{position:1.0,value:1.0};

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize)]
pub struct CurveRemap {
    pub points:[CurvePoint;MAX_CURVE_POINTS],
    pub count:u8,
}

impl Default for CurveRemap {
    fn default()->Self {
        let mut points=[EMPTY;MAX_CURVE_POINTS];
        points[0]=CurvePoint{position:0.0,value:0.0};
        points[1]=CurvePoint{position:0.25,value:0.1};
        points[2]=CurvePoint{position:0.75,value:0.9};
        Self{points,count:4}
    }
}

fn monotone_tangent(left:f32,right:f32)->f32 {
    if left*right<=0.0 {0.0} else {2.0*left*right/(left+right)}
}

impl CurveRemap {
    pub fn validate(&self)->Result<(),String> {
        let count=self.count as usize;
        if !(2..=MAX_CURVE_POINTS).contains(&count) {
            return Err("Curve Remap requires 2–8 control points".into());
        }
        let active=&self.points[..count];
        if active[0].position!=0.0 || active[count-1].position!=1.0 {
            return Err("Curve Remap endpoints must stay at 0 and 1".into());
        }
        for point in active {
            if !point.position.is_finite() || !point.value.is_finite()
                || !(0.0..=1.0).contains(&point.value) {
                return Err("Curve Remap positions must be finite and values in 0..1".into());
            }
        }
        if active.windows(2).any(|pair|pair[0].position>=pair[1].position) {
            return Err("Curve Remap positions must increase strictly".into());
        }
        Ok(())
    }

    /// Hermite interpolation: corner tangents flatten where neighboring
    /// slopes change sign; outputs are clamped to the two bracketing points
    /// so nonmonotone authored curves cannot overshoot the [0,1] range.
    pub fn sample(&self,input:f32)->f32 {
        let x=input.clamp(0.0,1.0);
        let count=self.count as usize;
        for i in 0..count-1 {
            let a=self.points[i];
            let b=self.points[i+1];
            if x>b.position {continue;}
            let width=b.position-a.position;
            let secant=(b.value-a.value)/width;
            let before=if i>0 {
                (a.value-self.points[i-1].value)/
                    (a.position-self.points[i-1].position)
            } else {secant};
            let after=if i+2<count {
                (self.points[i+2].value-b.value)/
                    (self.points[i+2].position-b.position)
            } else {secant};
            let left=if i==0 {secant}
                else {monotone_tangent(before,secant)};
            let right=if i+2==count {secant}
                else {monotone_tangent(secant,after)};
            let t=((x-a.position)/width).clamp(0.0,1.0);
            let t2=t*t;
            let t3=t2*t;
            let y=(2.0*t3-3.0*t2+1.0)*a.value
                +(t3-2.0*t2+t)*width*left
                +(-2.0*t3+3.0*t2)*b.value
                +(t3-t2)*width*right;
            return y.clamp(a.value.min(b.value),a.value.max(b.value))
                .clamp(0.0,1.0);
        }
        self.points[count-1].value
    }

    pub fn add_point(&mut self)->bool {
        let count=self.count as usize;
        if count>=MAX_CURVE_POINTS {return false;}
        let i=(0..count-1).max_by(|&a,&b|
            (self.points[a+1].position-self.points[a].position)
                .total_cmp(&(self.points[b+1].position-self.points[b].position)))
            .expect("a valid curve has at least two points");
        let mid=(self.points[i].position+self.points[i+1].position)*0.5;
        let value=self.sample(mid);
        for j in (i+1..count).rev() {
            self.points[j+1]=self.points[j];
        }
        self.points[i+1]=CurvePoint{position:mid,value};
        self.count+=1;
        true
    }

    pub fn remove_point(&mut self,index:usize)->bool {
        let count=self.count as usize;
        if index==0 || index+1>=count {return false;}
        for i in index..count-1 {
            self.points[i]=self.points[i+1];
        }
        self.points[count-1]=EMPTY;
        self.count-=1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linear_endpoints_are_exact_and_clamped() {
        let mut curve=CurveRemap::default();
        curve.count=2;
        curve.points[0].value=0.2;
        curve.points[1]=CurvePoint{position:1.0,value:0.8};
        curve.validate().unwrap();
        assert!((curve.sample(0.5)-0.5).abs()<1e-6);
        assert_eq!(curve.sample(-1.0),0.2);
        assert_eq!(curve.sample(2.0),0.8);
    }
    #[test]
    fn nonmonotone_points_remain_bounded_by_each_segment() {
        let mut curve=CurveRemap::default();
        curve.points[0].value=0.8;
        curve.points[1].value=0.1;
        curve.points[2].value=0.9;
        curve.points[3].value=0.2;
        curve.validate().unwrap();
        for step in 0..=1000 {
            let x=step as f32/1000.0;
            let y=curve.sample(x);
            assert!((0.0..=1.0).contains(&y));
            let pair=curve.points[..curve.count as usize]
                .windows(2).find(|p|x<=p[1].position).unwrap();
            assert!(y>=pair[0].value.min(pair[1].value)-0.000001);
            assert!(y<=pair[0].value.max(pair[1].value)+0.000001);
        }
    }
    #[test]
    fn insert_remove_and_roundtrip_preserve_validity() {
        let mut curve=CurveRemap::default();
        for _ in 4..MAX_CURVE_POINTS {assert!(curve.add_point());}
        assert!(!curve.add_point());
        curve.validate().unwrap();
        assert!(!curve.remove_point(0));
        assert!(!curve.remove_point(curve.count as usize-1));
        assert!(curve.remove_point(2));
        curve.validate().unwrap();
        let saved=serde_json::to_string(&curve).unwrap();
        let restored:CurveRemap=serde_json::from_str(&saved).unwrap();
        assert_eq!(restored,curve);
    }
    #[test]
    fn invalid_points_are_rejected() {
        let mut curve=CurveRemap::default();
        curve.points[1].position=f32::NAN;
        assert!(curve.validate().is_err());
        curve.points[1].position=curve.points[2].position;
        assert!(curve.validate().is_err());
        curve.points[1].position=0.25;
        curve.points[1].value=-0.1;
        assert!(curve.validate().is_err());
        curve.points[1].value=0.1;
        curve.count=9;
        assert!(curve.validate().is_err());
    }
}
