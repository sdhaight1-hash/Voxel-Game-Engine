//! Editable bounded multi-stop ramp. Values are serialized into the owning
//! graph node, not stored as GPU or tree runtime state. Color interpolation
//! preserves the existing RGB (display-space) material authoring convention.
use serde::{Deserialize, Serialize};

pub const MAX_COLOR_STOPS:usize=8;

/// Stored stop values are exact authoring parameters; no preprocessing is
/// applied before the legacy RGB graph evaluator samples them.
#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize)]
pub struct ColorStop {
    pub position:f32,
    pub rgb:[u8;3],
}

const EMPTY_STOP:ColorStop=ColorStop{position:1.0,rgb:[255,255,255]};

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize)]
pub struct ColorRamp {
    /// A fixed maximum keeps existing NodeKind:Copy and the graph's node UI
    /// architecture intact. Only the first count stops affect evaluation.
    pub stops:[ColorStop;MAX_COLOR_STOPS],
    pub count:u8,
}

impl Default for ColorRamp {
    fn default()->Self {
        let mut stops=[EMPTY_STOP;MAX_COLOR_STOPS];
        stops[0]=ColorStop{position:0.0,rgb:[24,38,36]};
        stops[1]=ColorStop{position:0.30,rgb:[64,101,66]};
        stops[2]=ColorStop{position:0.68,rgb:[139,158,85]};
        stops[3]=ColorStop{position:1.0,rgb:[223,206,139]};
        Self{stops,count:4}
    }
}

impl ColorRamp {
    pub fn validate(&self)->Result<(),String> {
        let count=self.count as usize;
        if !(2..=MAX_COLOR_STOPS).contains(&count) {
            return Err("Color Ramp requires 2–8 stops".into());
        }
        let stops=&self.stops[..count];
        if stops.first().unwrap().position!=0.0 ||
            stops.last().unwrap().position!=1.0 {
            return Err("Color Ramp endpoints must be 0 and 1".into());
        }
        for pair in stops.windows(2) {
            if !pair[0].position.is_finite() || !pair[1].position.is_finite() ||
                pair[0].position>=pair[1].position {
                return Err("Color Ramp stops must have increasing finite positions".into());
            }
        }
        Ok(())
    }

    pub fn sample(&self,value:f32)->[f32;3] {
        let t=value.clamp(0.0,1.0);
        let active=&self.stops[..self.count as usize];
        for pair in active.windows(2) {
            let (left,right)=(pair[0],pair[1]);
            if t<=right.position {
                let blend=((t-left.position)/(right.position-left.position))
                    .clamp(0.0,1.0);
                return std::array::from_fn(|i|
                    (left.rgb[i] as f32*(1.0-blend)+right.rgb[i] as f32*blend)/255.0);
            }
        }
        active.last().unwrap().rgb.map(|v|v as f32/255.0)
    }

    /// Insert a color between the two most widely separated existing stops.
    /// Add/remove only change the current node, not a referenced gradient.
    pub fn add_stop(&mut self)->bool {
        let count=self.count as usize;
        if count>=MAX_COLOR_STOPS {return false;}
        let index=(0..count-1).max_by(|&a,&b|
            (self.stops[a+1].position-self.stops[a].position)
                .total_cmp(&(self.stops[b+1].position-self.stops[b].position)))
            .expect("a validated ramp has two stops");
        let left=self.stops[index];
        let right=self.stops[index+1];
        let mid=ColorStop{
            position:(left.position+right.position)*0.5,
            rgb:std::array::from_fn(|i|
                ((left.rgb[i] as u16+right.rgb[i] as u16+1)/2) as u8),
        };
        for i in (index+1..count).rev() {
            self.stops[i+1]=self.stops[i];
        }
        self.stops[index+1]=mid;
        self.count+=1;
        true
    }

    /// The fixed endpoints cannot be removed; keep at least two stops.
    pub fn remove_stop(&mut self,index:usize)->bool {
        let count=self.count as usize;
        if index==0 || index+1>=count {return false;}
        for i in index..count-1 {
            self.stops[i]=self.stops[i+1];
        }
        self.stops[count-1]=EMPTY_STOP;
        self.count-=1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpolation_and_bounded_endpoints() {
        let mut ramp=ColorRamp::default();
        ramp.stops[0].rgb=[0,0,0];
        ramp.stops[1].rgb=[255,255,255];
        ramp.stops[1].position=0.5;
        ramp.stops[2].position=0.75;
        assert!(ramp.validate().is_ok());
        assert_eq!(ramp.sample(-1.0),[0.0;3]);
        assert_eq!(ramp.sample(0.25),[0.5;3]);
        assert_eq!(ramp.sample(1.5),
            ramp.stops[3].rgb.map(|c|c as f32/255.0));
    }

    #[test]
    fn add_remove_keep_sorted_limits_and_serde_roundtrip() {
        let mut ramp=ColorRamp::default();
        for _ in 4..MAX_COLOR_STOPS {assert!(ramp.add_stop());}
        assert!(!ramp.add_stop());
        assert_eq!(ramp.count,8);
        ramp.validate().unwrap();
        assert!(!ramp.remove_stop(0));
        assert!(!ramp.remove_stop(ramp.count as usize-1));
        assert!(ramp.remove_stop(3));
        ramp.validate().unwrap();
        let decoded:ColorRamp=serde_json::from_str(
            &serde_json::to_string(&ramp).unwrap()).unwrap();
        assert_eq!(decoded,ramp);
    }

    #[test]
    fn invalid_ramps_fail_validation() {
        let mut ramp=ColorRamp::default();
        ramp.count=1;
        assert!(ramp.validate().is_err());
        ramp.count=4;
        ramp.stops[1].position=f32::NAN;
        assert!(ramp.validate().is_err());
        ramp.stops[1].position=ramp.stops[2].position;
        assert!(ramp.validate().is_err());
        ramp.stops[1].position=0.3;
        ramp.stops[3].position=0.9;
        assert!(ramp.validate().is_err());
    }
}
