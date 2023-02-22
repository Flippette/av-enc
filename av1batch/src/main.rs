use evalexpr::eval;
use std::{env, ffi::OsStr, fs, path::Path, process::Command, thread};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const OP_PREFIX: &str = "op/av1";
const MAP_PREFIX: &str = "map";

fn main() -> Result<()> {
    let argv = env::args().collect::<Vec<_>>();
    env::set_current_dir(argv.get(1).unwrap_or(&".".to_string()))?;

    let cwd = env::current_dir()?;
    let cwd = cwd.to_str().unwrap();

    fs::create_dir_all(format!("{cwd}/{OP_PREFIX}"))?;
    fs::create_dir_all(format!("{cwd}/{MAP_PREFIX}"))?;

    for file in fs::read_dir(cwd)?
        .filter_map(|e| e.ok())
        .filter(|e| e.metadata().unwrap().is_file())
    {
        let path = file.path();
        let stem = path.file_stem().and_then(OsStr::to_str).unwrap();
        let ext = path.extension().and_then(OsStr::to_str).unwrap();

        let fps = eval(&ffprobe_query(&path, "avg_frame_rate")?)?.as_number()?;
        let (pix_fmt, bit_depth) = match ffprobe_query(&path, "pix_fmt") {
            Ok(pf) if pf.contains("10") => ("yuv420p10le", 10),
            _ => ("yuv420p", 8),
        };

        Command::new("ffmpeg")
            .arg("-y")
            .args(["-hwaccel", "auto"])
            .args(["-i", path.to_str().ok_or("invalid path!")?])
            .args(["-pix_fmt", pix_fmt])
            .args(["-map", "0:v"])
            .args(["-map", "0:a:m:language:jpn"])
            .args(["-map", "0:s:m:language:eng"])
            .arg(&format!("{cwd}/{MAP_PREFIX}/{stem}.{ext}"))
            .spawn()?
            .wait()?;

        #[rustfmt::skip]
            Command::new("av1an")
                .args(["-i", &format!("{cwd}/{MAP_PREFIX}/{stem}.{ext}")])
                .args(["-o", &format!("{cwd}/{OP_PREFIX}/{stem}.{ext}")])
                .args([
                    "--video-params",
                    &format!(
                        "\"{}\"",
                        [
                            "--profile=0",
                            "--passes=2",
                            "--threads=2",
                            "--lag-in-frames=96",
                            "--end-usage=q",
                            "--cq-level=23",
                            "--enable-fwd-kf=1",
                            "--enable-keyframe-filtering=2",
                            &format!("--kf-max-dist={}", fps * 10.0),
                            "--cpu-used=3",
                            &format!("--bit-depth={bit_depth}"),
                            "--tune-content=animation",
                            "--tune=butteraugli",
                            "--tile-columns=1",
                            "--tile-rows=0",
                            "--aq-mode=1",
                            "--deltaq-mode=1",
                            "--enable-qm=1",
                            "--min-q=1",
                            "--quant-b-adapt=1",
                            "--disable-trellis-quant=0",
                            "--arnr-strength=0",
                            "--arnr-maxframes=15",
                            "--sharpness=2",
                            "--enable-warped-motion=0",
                        ]
                        .join(" ")
                    ),
                ])
                .args(["--target-quality", "92"])
                .args([
                    "--audio-params",
                    &format!("\"{}\"",
                        [
                            "-c:a", "libopus",
                            "-b:a", "128k"
                        ]
                        .join(" ")
                    ),
                ])
                .args(["--pix-format", pix_fmt])
                .args(["--encoder", "aom"])
                .arg("--resume")
                .args([
                    "--workers",
                    &(thread::available_parallelism()?.get() / 2).to_string(),
                ])
                .args(["--set-thread-affinity", "2"])
                .args(["--passes", "2"])
                .args(["--concat", "mkvmerge"])
                .arg("--verbose")
                .spawn()?
                .wait()?;
    }

    Ok(())
}

fn ffprobe_query(file: &Path, entry: &str) -> Result<String> {
    let stdout = std::string::String::from_utf8(
        std::process::Command::new("ffprobe")
            .args(["-loglevel", "panic"])
            .args(["-select_streams", "v"])
            .args(["-show_entries", &format!("stream={}", entry)])
            .arg(file)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();

    Ok(stdout
        .lines()
        .find(|line| line.starts_with(entry))
        .ok_or(format!("not found: {entry}"))?
        .split('=')
        .nth(1)
        .ok_or(format!("not found: {entry}"))?
        .to_string())
}
