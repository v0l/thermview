//! Export the actual parser outputs for independent radiometric validation.
//! cargo run --example temperature_dump -- INPUT OUTPUT_DIRECTORY [1,2,3]
//! Frame numbers are one-based, matching the ThermView interface.
use std::{fs, path::Path};
use thermview::{ThermalImage, image::Format, video::Video};

fn dump(image: &ThermalImage, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output)?;
    fs::write(
        output.join("temperature.f32"),
        image.celsius.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>(),
    )?;
    let p = image.params;
    let calibration = match &image.radiometry {
        Some(r) => {
            fs::write(
                output.join("sensor.u16"),
                r.counts.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>(),
            )?;
            let c = r.planck;
            format!(r#"{{"r1":{},"b":{},"f":{},"o":{},"r2":{}}}"#, c.r1, c.b, c.f, c.o, c.r2)
        }
        None => "null".to_owned(),
    };
    let environment = image
        .radiometry
        .as_ref()
        .map(|r| {
            let e = r.environment;
            format!(
                r#"{{"ATA1":{},"ATA2":{},"ATB1":{},"ATB2":{},"ATX":{},"IRT":{},"IRWTemp":{}}}"#,
                e.alpha1,
                e.alpha2,
                e.beta1,
                e.beta2,
                e.x,
                e.window_transmission,
                e.window_temperature
            )
        })
        .unwrap_or_else(|| "null".to_owned());
    fs::write(
        output.join("metadata.json"),
        format!(
            concat!(
                "{{\"format\":\"{}\",\"width\":{},\"height\":{},",
                "\"emissivity\":{},\"distance\":{},\"reflected\":{},",
                "\"air\":{},\"humidity\":{},\"planck\":{},\"environment\":{}}}\n"
            ),
            image.format.label(),
            image.width,
            image.height,
            p.emissivity,
            p.distance,
            p.reflected,
            p.air,
            p.humidity,
            calibration,
            environment
        ),
    )?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        return Err("usage: temperature_dump INPUT OUTPUT_DIRECTORY [1,2,3]".into());
    }
    let source = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    let bytes = fs::read(source)?;
    let name = source.file_name().unwrap_or_default().to_string_lossy();
    if thermview::formats::detect(&bytes, Some(&name))? == Format::FlirCsq {
        let video = Video::open(bytes)?;
        let frames = args.get(2).map(|v| v.to_string_lossy()).unwrap_or("1".into());
        for number in frames.split(',') {
            let frame: usize = number.parse()?;
            let folder = output.join(format!("frame-{frame:05}"));
            match video.frame(frame.checked_sub(1).ok_or("frames start at 1")?) {
                Ok(image) => dump(&image, &folder)?,
                Err(error) => {
                    fs::create_dir_all(&folder)?;
                    fs::write(folder.join("error.txt"), error.to_string())?;
                }
            }
        }
    } else {
        dump(&thermview::parse(&bytes, Some(&name))?, output)?;
    }
    Ok(())
}
