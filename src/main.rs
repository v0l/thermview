use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use thermview::doc::Doc;
use thermview::palette::{Mapping, Overscan, Palette, Unit};

#[derive(Parser)]
#[command(version, about, args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    Info {
        file: PathBuf,
    },
    Render {
        file: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "inferno")]
        palette: String,
        #[arg(long, default_value_t = 3)]
        scale: u32,
        #[arg(long)]
        fahrenheit: bool,
        #[arg(long, value_name = "LOW:HIGH")]
        range: Option<String>,
        #[arg(long, default_value = "linear")]
        mapping: String,
        #[arg(long, default_value = "clip")]
        overscan: String,
        #[arg(long)]
        invert: bool,
        #[arg(long = "point", value_name = "X,Y")]
        points: Vec<String>,
        #[arg(long)]
        bare: bool,
    },
}

fn open(file: &Path) -> Result<Doc, String> {
    let loaded = thermview::host::read(file)?;
    let img = thermview::parse(&loaded.bytes, Some(&loaded.name)).map_err(|e| e.to_string())?;
    Ok(Doc::new(img, &loaded.name, loaded.modified))
}

fn info(file: &Path) -> Result<(), String> {
    let doc = open(file)?;
    let i = &doc.image;
    let s = &i.stats;
    println!("format      {}", i.format.label());
    println!("size        {} x {}", i.width, i.height);
    println!("coldest     {:.2} °C at {}, {}", s.min.celsius, s.min.x, s.min.y);
    println!("hottest     {:.2} °C at {}, {}", s.max.celsius, s.max.x, s.max.y);
    println!("emissivity  {:.3}", i.params.emissivity);
    println!("distance    {:.2} m", i.params.distance);
    println!("reflected   {:.2} °C", i.params.reflected);
    println!("air         {:.2} °C", i.params.air);
    println!("humidity    {:.0} %", i.params.humidity);
    println!("atm trans   {:.4}", i.atm_trans);
    if let Some(r) = &i.radiometry {
        let p = r.planck;
        println!("planck      R1 {} B {} F {} O {} R2 {}", p.r1, p.b, p.f, p.o, p.r2);
    }
    let c = &i.camera;
    for (k, v) in [
        ("camera", c.make_model()),
        ("lens", c.lens()),
        ("software", c.software.clone()),
        ("serial", c.serial.clone()),
        ("taken", doc.stamp.clone()),
    ] {
        if let Some(v) = v {
            println!("{k:<11} {v}");
        }
    }
    if let (Some(lat), Some(lon)) = (c.latitude, c.longitude) {
        println!("position    {lat:.5}, {lon:.5}");
    }
    Ok(())
}

fn pick<T: Copy>(
    all: &[T],
    label: impl Fn(T) -> &'static str,
    name: &str,
    what: &str,
) -> Result<T, String> {
    all.iter()
        .copied()
        .find(|v| {
            label(*v).eq_ignore_ascii_case(name)
                || label(*v).starts_with(&name.to_ascii_lowercase())
        })
        .ok_or_else(|| {
            let names: Vec<&str> = all.iter().map(|v| label(*v)).collect();
            format!("unknown {what} {name}, one of {}", names.join(", "))
        })
}

#[allow(clippy::too_many_arguments)]
fn render(
    file: &Path,
    out: Option<PathBuf>,
    palette: &str,
    scale: u32,
    fahrenheit: bool,
    range: Option<String>,
    mapping: &str,
    overscan: &str,
    invert: bool,
    points: &[String],
    bare: bool,
) -> Result<(), String> {
    let mut doc = open(file)?;
    doc.look.palette =
        Palette::named(palette).ok_or_else(|| format!("unknown palette {palette}"))?;
    doc.look.mapping = pick(&Mapping::ALL, Mapping::label, mapping, "mapping")?;
    doc.look.overscan = pick(&Overscan::ALL, Overscan::label, overscan, "overscan")?;
    doc.look.inverted = invert;
    doc.scale = scale.clamp(1, 16);
    doc.unit = if fahrenheit { Unit::Fahrenheit } else { Unit::Celsius };
    if let Some(r) = range {
        let (lo, hi) = r.split_once(':').ok_or("range is LOW:HIGH")?;
        let num = |s: &str| s.trim().parse::<f64>().map_err(|_| format!("bad number {s}"));
        doc.look.min = doc.unit.to_celsius(num(lo)?) as f32;
        doc.look.max = doc.unit.to_celsius(num(hi)?) as f32;
    }
    for p in points {
        let (x, y) = p.split_once(',').ok_or("point is X,Y")?;
        let n = |s: &str| s.trim().parse::<usize>().map_err(|_| format!("bad pixel {s}"));
        doc.add_point(n(x)?, n(y)?);
    }
    if bare {
        doc.overlays = thermview::doc::Overlays { extremes: false, info: false, time: false };
    }
    let target = out.unwrap_or_else(|| file.with_file_name(thermview::export::file_name(&doc)));
    std::fs::write(&target, thermview::export::png(&doc))
        .map_err(|e| format!("{}: {e}", target.display()))?;
    println!("wrote {}", target.display());
    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        None => thermview::app::run(cli.file).map_err(|e| e.to_string()),
        Some(Command::Info { file }) => info(&file),
        Some(Command::Render {
            file,
            out,
            palette,
            scale,
            fahrenheit,
            range,
            mapping,
            overscan,
            invert,
            points,
            bare,
        }) => render(
            &file, out, &palette, scale, fahrenheit, range, &mapping, &overscan, invert, &points,
            bare,
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("thermview: {e}");
            ExitCode::FAILURE
        }
    }
}
