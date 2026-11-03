//! Embedded RGBA8 bitmap assets for portable material graphs.
//! PNG decoding occurs at import time, never inside the per-pixel evaluator.
use serde::{Serialize,Deserialize};

pub const MAX_SIDE:u32=512;
pub const MAX_IMAGES:usize=16;
pub const MAX_PIXELS_BYTES:usize=8*1024*1024;
pub const MAX_PNG_BYTES:usize=4*1024*1024;

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub struct BitmapAsset {
    pub id:u64,
    pub name:String,
    pub width:u16,
    pub height:u16,
    pub rgba:Vec<u8>,
    pub fingerprint:u64,
}

/// Stable, deterministic FNV-1a cache checksum (not a security hash).
pub fn fingerprint(w:u16,h:u16,pixels:&[u8])->u64 {
    let mut digest=0xcbf29ce484222325u64;
    for byte in w.to_le_bytes().into_iter()
        .chain(h.to_le_bytes()).chain(pixels.iter().copied()) {
        digest=(digest ^ byte as u64).wrapping_mul(0x100000001b3);
    }
    digest
}

impl BitmapAsset {
    pub fn new(id:u64,name:String,width:u32,height:u32,rgba:Vec<u8>)
        ->Result<Self,String> {
        if id==0||width==0||height==0||width>MAX_SIDE||height>MAX_SIDE
            ||rgba.len()!=width as usize*height as usize*4 {
            return Err("Embedded bitmap requires 1..512 RGBA8 pixels per axis".into());
        }
        if name.is_empty()||name.len()>80||name.contains('/')
            ||name.contains('\\')||name.chars().any(char::is_control) {
            return Err("Embedded bitmap needs a filename label up to 80 bytes".into());
        }
        let (width,height)=(width as u16,height as u16);
        Ok(Self{id,name,width,height,
            fingerprint:fingerprint(width,height,&rgba),rgba})
    }

    pub fn validate(&self)->Result<(),String> {
        if self.id==0||self.width==0||self.height==0
            ||self.width as u32>MAX_SIDE||self.height as u32>MAX_SIDE
            ||self.rgba.len()!=self.width as usize*self.height as usize*4
            ||self.name.is_empty()||self.name.len()>80||self.name.contains('/')
            ||self.name.contains('\\')||self.name.chars().any(char::is_control)
            ||self.fingerprint!=fingerprint(self.width,self.height,&self.rgba) {
            return Err("Invalid embedded bitmap data or content fingerprint".into());
        }
        Ok(())
    }

    /// Repeat UV on both axes. Bilinear filtering joins adjacent tile edges.
    pub fn sample(&self,u:f32,v:f32,bilinear:bool)->[f32;4] {
        let (w,h)=(self.width as i32,self.height as i32);
        let (x,y)=(u.rem_euclid(1.0)*w as f32,
                   v.rem_euclid(1.0)*h as f32);
        let pixel=|px:i32,py:i32| {
            let col=px.rem_euclid(w) as usize;
            let row=py.rem_euclid(h) as usize;
            let pos=(row*w as usize+col)*4;
            std::array::from_fn(|c|self.rgba[pos+c] as f32/255.0)
        };
        if !bilinear {return pixel(x.floor() as i32,y.floor() as i32);}
        let (x,y)=(x-0.5,y-0.5);
        let (ix,iy)=(x.floor() as i32,y.floor() as i32);
        let (tx,ty)=(x-ix as f32,y-iy as f32);
        let a:[f32;4]=pixel(ix,iy);
        let b:[f32;4]=pixel(ix+1,iy);
        let c:[f32;4]=pixel(ix,iy+1);
        let d:[f32;4]=pixel(ix+1,iy+1);
        std::array::from_fn(|i|
            (a[i]*(1.0-tx)+b[i]*tx)*(1.0-ty)
            +(c[i]*(1.0-tx)+d[i]*tx)*ty)
    }
}

pub fn validate_assets(images:&[BitmapAsset])->Result<(),String> {
    if images.len()>MAX_IMAGES {return Err("Too many embedded bitmap assets".into());}
    let mut ids=std::collections::HashSet::new();
    let mut bytes=0usize;
    for image in images {
        image.validate()?;
        if !ids.insert(image.id) {return Err("Duplicate bitmap asset ID".into());}
        bytes=bytes.checked_add(image.rgba.len()).ok_or("Image bytes overflow")?;
        if bytes>MAX_PIXELS_BYTES {return Err("Embedded bitmap limit: 8 MiB".into());}
    }
    Ok(())
}

/// Decode PNG input with explicit compressed-size, decoder-memory and pixel limits.
pub fn decode_png(bytes:&[u8])->Result<(u32,u32,Vec<u8>),String> {
    if bytes.len()>MAX_PNG_BYTES {return Err("PNG exceeds 4 MiB".into());}
    let mut limits=png::Limits::default();
    limits.bytes=8*1024*1024;
    let mut decoder=png::Decoder::new_with_limits(
        std::io::Cursor::new(bytes),limits);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_transformations(
        png::Transformations::EXPAND|png::Transformations::STRIP_16);
    let mut reader=decoder.read_info().map_err(|e|format!("PNG header: {e}"))?;
    let info=reader.info();
    let (w,h)=(info.width,info.height);
    if w==0||h==0||w>MAX_SIDE||h>MAX_SIDE {
        return Err("PNG dimensions must be 1..512 per axis".into());
    }
    if info.animation_control.is_some() {
        return Err("Animated PNGs are not supported".into());
    }
    let byte_count=reader.output_buffer_size().ok_or("PNG output overflow")?;
    if byte_count>MAX_SIDE as usize*MAX_SIDE as usize*4 {
        return Err("PNG output exceeds decoded-image limit".into());
    }
    let mut buf=vec![0u8;byte_count];
    let frame=reader.next_frame(&mut buf)
        .map_err(|e|format!("PNG decode: {e}"))?;
    if frame.width!=w||frame.height!=h||frame.bit_depth!=png::BitDepth::Eight {
        return Err("Unsupported PNG format".into());
    }
    let src=&buf[..frame.buffer_size()];
    let mut rgba=Vec::with_capacity(w as usize*h as usize*4);
    match frame.color_type {
        png::ColorType::Rgba=>rgba.extend_from_slice(src),
        png::ColorType::Rgb=>for p in src.chunks_exact(3) {
            rgba.extend_from_slice(&[p[0],p[1],p[2],255]);
        },
        png::ColorType::Grayscale=>for &g in src {
            rgba.extend_from_slice(&[g,g,g,255]);
        },
        png::ColorType::GrayscaleAlpha=>for p in src.chunks_exact(2) {
            rgba.extend_from_slice(&[p[0],p[0],p[0],p[1]]);
        },
        _=>return Err("Unsupported PNG color format".into()),
    }
    if rgba.len()!=w as usize*h as usize*4 {
        return Err("PNG RGBA conversion length mismatch".into());
    }
    Ok((w,h,rgba))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tiled_bilinear_sample_is_repeatable_and_alpha_is_independent() {
        let a=BitmapAsset::new(1,"test.png".into(),2,1,
            vec![255,0,0,0,0,255,0,255]).unwrap();
        assert_eq!(a.sample(0.25,0.5,false),[1.0,0.0,0.0,0.0]);
        assert_eq!(a.sample(0.75,0.5,false),[0.0,1.0,0.0,1.0]);
        assert!((a.sample(0.0,0.5,true)[3]-0.5).abs()<1e-6);
        for (u,v) in [(0.0,0.0),(-0.5,0.5),(0.25,0.75)] {
            assert_eq!(a.sample(u,v,true),a.sample(u+1.0,v,true));
            assert_eq!(a.sample(u,v,true),a.sample(u,v+1.0,true));
        }
    }
    #[test]
    fn png_decoder_and_limits() {
        let mut png=Vec::new();
        {
            let mut e=png::Encoder::new(&mut png,2,1);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            let mut writer=e.write_header().unwrap();
            writer.write_image_data(&[255,0,0,255,0,30,80,64]).unwrap();
        }
        assert_eq!(decode_png(&png).unwrap(),
            (2,1,vec![255,0,0,255,0,30,80,64]));
        assert!(decode_png(&vec![0;MAX_PNG_BYTES+1]).is_err());
        assert!(decode_png(&[1,2,3]).is_err());
    }
    #[test]
    fn changes_to_embedded_pixels_require_new_fingerprint() {
        let mut a=BitmapAsset::new(1,"b.png".into(),1,1,vec![1,2,3,4]).unwrap();
        a.rgba[0]=7;
        assert!(a.validate().is_err());
        assert!(BitmapAsset::new(1,"../bad".into(),1,1,vec![1,2,3,4]).is_err());
    }
}
