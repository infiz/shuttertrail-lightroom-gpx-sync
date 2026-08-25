use serde_json::Value;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const READY_PREFIX: &str = "{ready";

pub struct ExifToolManager {
    candidates: Vec<PathBuf>,
    process: Option<ExifToolProcess>,
}

impl ExifToolManager {
    pub fn new(resource_dir: PathBuf) -> Self {
        let mut candidates = Vec::new();
        if let Some(path) = env::var_os("SHUTTERTRAIL_EXIFTOOL") {
            candidates.push(PathBuf::from(path));
        }

        #[cfg(target_os = "windows")]
        {
            candidates.push(resource_dir.join("resources/exiftool/windows/exiftool.exe"));
            candidates.push(resource_dir.join("exiftool/windows/exiftool.exe"));
        }
        #[cfg(target_os = "macos")]
        {
            candidates.push(resource_dir.join("resources/exiftool/macos/exiftool"));
            candidates.push(resource_dir.join("exiftool/macos/exiftool"));
        }

        #[cfg(target_os = "windows")]
        candidates.push(PathBuf::from(
            "shuttertrail-lightroom-gpx-sync.lrplugin/bin/windows/exiftool.exe",
        ));
        #[cfg(target_os = "macos")]
        {
            candidates.push(PathBuf::from("/opt/homebrew/bin/exiftool"));
            candidates.push(PathBuf::from("/usr/local/bin/exiftool"));
        }
        candidates.push(PathBuf::from(if cfg!(target_os = "windows") {
            "exiftool.exe"
        } else {
            "exiftool"
        }));

        Self {
            candidates,
            process: None,
        }
    }

    pub fn execute(&mut self, args: &[String]) -> Result<String, String> {
        if self.process.is_none() {
            self.process = Some(self.start_process()?);
        }

        match self
            .process
            .as_mut()
            .expect("process initialized")
            .execute(args)
        {
            Ok(output) => Ok(output),
            Err(first_error) => {
                self.process = None;
                let mut replacement = self.start_process().map_err(|restart| {
                    format!("{first_error}\nExifTool restart failed: {restart}")
                })?;
                let output = replacement.execute(args);
                self.process = Some(replacement);
                output
            }
        }
    }

    fn execute_without_retry(&mut self, args: &[String]) -> Result<String, String> {
        if self.process.is_none() {
            self.process = Some(self.start_process()?);
        }
        let result = self
            .process
            .as_mut()
            .expect("process initialized")
            .execute(args);
        if result.is_err() {
            self.process = None;
        }
        result
    }

    pub fn version(&mut self) -> Result<String, String> {
        self.execute(&["-ver".into()])
            .map(|value| value.trim().to_string())
    }

    pub fn read_json(&mut self, paths: &[PathBuf]) -> Result<Vec<Value>, String> {
        let mut args = vec![
            "-j".into(),
            "-n".into(),
            "-charset".into(),
            "filename=UTF8".into(),
            "-DateTimeOriginal".into(),
            "-SubSecTimeOriginal".into(),
            "-OffsetTimeOriginal".into(),
            "-GPSLatitude".into(),
            "-GPSLongitude".into(),
            "-GPSAltitude".into(),
            "-Make".into(),
            "-Model".into(),
            "-SerialNumber".into(),
            "-InternalSerialNumber".into(),
            "-BodySerialNumber".into(),
            "-FileType".into(),
        ];
        args.extend(
            paths
                .iter()
                .map(|path| path_argument(path.as_path()))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let output = self.execute(&args)?;
        serde_json::from_str(&output)
            .map_err(|error| format!("Could not decode ExifTool metadata: {error}\n{output}"))
    }

    pub fn write_gps(
        &mut self,
        path: &Path,
        latitude: f64,
        longitude: f64,
        altitude: Option<f64>,
    ) -> Result<String, String> {
        let mut args = vec![
            "-overwrite_original_in_place".into(),
            "-P".into(),
            "-n".into(),
            format!("-EXIF:GPSLatitude={:.9}", latitude.abs()),
            format!(
                "-EXIF:GPSLatitudeRef={}",
                if latitude < 0.0 { "S" } else { "N" }
            ),
            format!("-EXIF:GPSLongitude={:.9}", longitude.abs()),
            format!(
                "-EXIF:GPSLongitudeRef={}",
                if longitude < 0.0 { "W" } else { "E" }
            ),
            "-EXIF:GPSMapDatum=WGS-84".into(),
        ];
        if let Some(value) = altitude {
            args.push(format!("-EXIF:GPSAltitude={:.3}", value.abs()));
            args.push(format!(
                "-EXIF:GPSAltitudeRef={}",
                if value < 0.0 { 1 } else { 0 }
            ));
        }
        args.push(path_argument(path)?);

        // Never blindly retry a file mutation. If ExifTool exits after changing
        // the file but before acknowledging the request, the caller must verify
        // the file and decide whether to restore its backup.
        let output = self.execute_without_retry(&args)?;
        if output.to_ascii_lowercase().contains("error:")
            || output.contains("0 image files updated")
        {
            return Err(output.trim().to_string());
        }
        Ok(output.trim().to_string())
    }

    fn start_process(&self) -> Result<ExifToolProcess, String> {
        let mut failures = Vec::new();
        for candidate in &self.candidates {
            match ExifToolProcess::start(candidate) {
                Ok(process) => return Ok(process),
                Err(error) => failures.push(format!("{}: {error}", candidate.display())),
            }
        }
        Err(format!(
            "ExifTool was not found or could not be started. Set SHUTTERTRAIL_EXIFTOOL or install ExifTool.\n{}",
            failures.join("\n")
        ))
    }
}

struct ExifToolProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl ExifToolProcess {
    fn start(executable: &Path) -> Result<Self, String> {
        let mut child = Command::new(executable)
            .args(["-config", "", "-stay_open", "True", "-@", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "ExifTool stdin was unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "ExifTool stdout was unavailable".to_string())?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    fn execute(&mut self, args: &[String]) -> Result<String, String> {
        let id = self.next_id;
        self.next_id += 1;
        for argument in args {
            if argument.contains(['\n', '\r']) {
                return Err("ExifTool arguments may not contain line breaks".into());
            }
            writeln!(self.stdin, "{argument}").map_err(|error| error.to_string())?;
        }
        writeln!(self.stdin, "-execute{id}").map_err(|error| error.to_string())?;
        self.stdin.flush().map_err(|error| error.to_string())?;

        let expected = format!("{READY_PREFIX}{id}}}");
        let mut output = String::new();
        loop {
            let mut line = String::new();
            let bytes = self
                .stdout
                .read_line(&mut line)
                .map_err(|error| error.to_string())?;
            if bytes == 0 {
                return Err(format!(
                    "ExifTool exited before completing request {id} (status: {:?})",
                    self.child.try_wait().ok().flatten()
                ));
            }
            if line.trim_end() == expected {
                break;
            }
            output.push_str(&line);
        }
        Ok(output)
    }
}

impl Drop for ExifToolProcess {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "-stay_open\nFalse");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

fn path_argument(path: &Path) -> Result<String, String> {
    let value = path.to_string_lossy().to_string();
    if value.contains(['\n', '\r']) {
        return Err(format!(
            "Paths containing line breaks are unsupported: {}",
            path.display()
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    #[ignore = "requires a system ExifTool executable"]
    fn persistent_worker_writes_and_reads_embedded_gps() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let source = manifest_dir.join("icons/icon.png");
        let target = env::temp_dir().join(format!(
            "shuttertrail-exiftool-test-{}.png",
            Uuid::new_v4().simple()
        ));
        fs::copy(&source, &target).unwrap();

        let mut manager = ExifToolManager::new(PathBuf::from("/nonexistent"));
        assert!(!manager.version().unwrap().is_empty());
        manager
            .write_gps(&target, 37.7749, -122.4194, Some(16.0))
            .unwrap();
        let metadata = manager.read_json(std::slice::from_ref(&target)).unwrap();
        let object = metadata[0].as_object().unwrap();
        assert!((object["GPSLatitude"].as_f64().unwrap() - 37.7749).abs() < 0.000001);
        assert!((object["GPSLongitude"].as_f64().unwrap() + 122.4194).abs() < 0.000001);

        fs::remove_file(target).unwrap();
    }
}
