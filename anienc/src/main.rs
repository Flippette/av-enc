use camino::{Utf8Path, Utf8PathBuf};
use eyre::{eyre, Result};
use glob::glob;
use std::{
    env,
    process::{Command, Stdio},
    thread,
};
use tracing::{info, warn};

fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt().compact().init();

    let threads = thread::available_parallelism()?.get();

    for path in env::args()
        .skip(1)
        .flat_map(|arg| glob(&arg))
        .flat_map(|p| p.into_iter())
    {
        let path: Utf8PathBuf = match path {
            Ok(p) => match Utf8PathBuf::try_from(p.clone()) {
                Ok(up) => match up.canonicalize_utf8() {
                    Ok(cup) if cup.is_file() => {
                        info!("encoding file {cup}.");
                        cup
                    }
                    Ok(cup) if cup.is_dir() => continue,
                    Ok(cup) => {
                        warn!("path {cup} does not exist, skipping.");
                        continue;
                    }
                    Err(e) => {
                        warn!("failed to canonicalize path {up} with error {e:?}, skipping.");
                        continue;
                    }
                },
                Err(e) => {
                    warn!("failed to parse {p:?} into unicode with error {e:?}, skipping.");
                    continue;
                }
            },
            Err(e) => {
                warn!("glob error encountered, skipping. {e:?}");
                continue;
            }
        };

        let stem = path.file_stem().unwrap_or_default();
        let ext = path.extension().unwrap_or_default();

        info!("searching for optimal crf...");
        let crf = crf_query(&path)?;

        info!("crf search complete, encoding...");
        Command::new("ffmpeg")
            .arg("-y")
            .args(["-hwaccel", "auto"])
            .args(["-i", path.as_ref()])
            .args(["-pix_fmt", "yuv420p10le"])
            .args(["-c:v", "libsvtav1"])
            .args([
                "-svtav1-params",
                &[
                    "preset=4",
                    "profile=0",
                    "input-depth=10",
                    &format!("lp={threads}"),
                    &format!("pin={threads}"),
                    &format!("crf={crf}"),
                    "aq-mode=1",
                    "enable-qm=1",
                    &format!("keyint={}", (framerate(&path)? * 10.0).round()),
                    "irefresh-type=2",
                    "scd=1",
                    "lookahead=120",
                    "enable-tf=0",
                    "enable-overlays=1",
                    "tune=0",
                ]
                .join(":"),
            ])
            .args(["-c:a", "libopus"])
            .args(["-b:a", "128k"])
            .args(["-map", "0:v"])
            .args(["-map", "0:a:m:language:jpn"])
            .args(["-map", "0:s:m:language:eng"])
            .arg(format!("{}/{stem}.op.{ext}", path.parent().unwrap()))
            .spawn()?
            .wait()?;
    }

    Ok(())
}

fn ffprobe_query(file: &Utf8Path, stream: &str, entry: &str) -> Result<String> {
    String::from_utf8(
        Command::new("ffprobe")
            .args(["-loglevel", "panic"])
            .args(["-select_streams", stream])
            .args(["-show_entries", &format!("stream={entry}")])
            .arg(file)
            .output()
            .unwrap()
            .stdout,
    )?
    .lines()
    .find(|line| line.starts_with(entry))
    .ok_or(eyre!("property query on {file} failed: not found"))?
    .split('=')
    .last()
    .ok_or(eyre!("property query on {file} failed: parse failure"))
    .map(String::from)
}

fn framerate(file: &Utf8Path) -> Result<f64> {
    let res = ffprobe_query(file, "v", "avg_frame_rate")?;
    let mut res = res.split('/').take(2).map(str::parse::<f64>);

    let numerator = res.next().unwrap()?;
    let denominator = res.next().unwrap()?;

    Ok(numerator / denominator)
}

fn crf_query(file: &Utf8Path) -> Result<u32> {
    String::from_utf8(
        Command::new("ab-av1")
            .arg("crf-search")
            .args(["-e", "libsvtav1"])
            .args(["-i", file.as_ref()])
            .args(["--pix-format", "yuv420p10le"])
            .args(["--min-vmaf", "92"])
            .args(["--min-samples", "2"])
            .args(["--vmaf-scale", "none"])
            .args(["--preset", "5"])
            .args(["--keyint", &(framerate(file)? * 10.0).round().to_string()])
            .args(["--scd", "true"])
            .args([
                "--svt",
                &[
                    "tune=0",
                    "enable-overlays=1",
                    "enable-tf=0",
                    "lookahead=120",
                    "irefresh-type=2",
                    "enable-qm=1",
                    "aq-mode=1",
                    "input-depth=10",
                    "profile=0",
                ]
                .join(":"),
            ])
            .args(["--cache", "false"])
            .args([
                "--temp-dir",
                &format!(
                    "{}/.tmp.{}",
                    file.parent().map(|p| p.as_ref()).unwrap_or("/"),
                    file.file_stem().unwrap_or("")
                ),
            ])
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()?
            .stdout,
    )?
    .lines()
    .find(|line| line.starts_with("crf"))
    .ok_or_else(|| eyre!("crf query on {file} failed!"))?
    .split_whitespace()
    .nth(1)
    .ok_or_else(|| eyre!("crf query on {file} failed: failed to parse ab-av1 output"))?
    .parse()
    .map_err(|e| eyre!("crf query on {file} failed: {e:?}"))
}
