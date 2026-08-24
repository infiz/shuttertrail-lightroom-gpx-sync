use chrono::{DateTime, Utc};
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct TrackPoint {
    pub epoch_millis: i64,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub source: String,
}

#[derive(Default)]
struct PendingPoint {
    latitude: f64,
    longitude: f64,
    altitude: Option<f64>,
    time: Option<DateTime<Utc>>,
}

pub fn parse_files(paths: &[PathBuf]) -> Result<(Vec<TrackPoint>, Vec<String>), String> {
    let mut points = Vec::new();
    let mut warnings = Vec::new();
    for path in paths {
        match parse_file(path) {
            Ok(mut parsed) => points.append(&mut parsed),
            Err(error) => warnings.push(format!("{}: {error}", path.display())),
        }
    }
    points.sort_by(|left, right| {
        left.epoch_millis
            .cmp(&right.epoch_millis)
            .then_with(|| left.source.cmp(&right.source))
    });
    if points.is_empty() {
        return Err(if warnings.is_empty() {
            "No timestamped GPX points were found".into()
        } else {
            format!("No usable GPX points were found:\n{}", warnings.join("\n"))
        });
    }
    Ok((points, warnings))
}

fn parse_file(path: &Path) -> Result<Vec<TrackPoint>, String> {
    let xml = fs::read(path).map_err(|error| error.to_string())?;
    let mut reader = Reader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut points = Vec::new();
    let mut pending: Option<PendingPoint> = None;
    let mut active_child: Option<String> = None;

    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| error.to_string())?
        {
            Event::Start(element) => {
                let name = element.local_name();
                if matches!(name.as_ref(), "trkpt" | "rtept" | "wpt") {
                    pending = Some(point_from_attributes(&element)?);
                    active_child = None;
                } else if pending.is_some() && matches!(name.as_ref(), "time" | "ele") {
                    active_child = Some(name.as_ref().to_string());
                }
            }
            Event::Text(text) => {
                if let (Some(point), Some(child)) = (&mut pending, &active_child) {
                    let value = text.xml10_content();
                    if child == "ele" {
                        point.altitude = value.parse::<f64>().ok();
                    } else if child == "time" {
                        point.time = DateTime::parse_from_rfc3339(&value)
                            .ok()
                            .map(|time| time.with_timezone(&Utc));
                    }
                }
            }
            Event::End(element) => {
                let name = element.local_name();
                if matches!(name.as_ref(), "trkpt" | "rtept" | "wpt") {
                    if let Some(point) = pending.take()
                        && let Some(time) = point.time
                    {
                        points.push(TrackPoint {
                            epoch_millis: time.timestamp_millis(),
                            latitude: point.latitude,
                            longitude: point.longitude,
                            altitude: point.altitude,
                            source: path.display().to_string(),
                        });
                    }
                    active_child = None;
                } else if matches!(name.as_ref(), "time" | "ele") {
                    active_child = None;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(points)
}

fn point_from_attributes(element: &BytesStart<'_>) -> Result<PendingPoint, String> {
    let mut latitude = None;
    let mut longitude = None;
    for attribute in element.attributes().with_checks(false) {
        let attribute = attribute.map_err(|error| error.to_string())?;
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|error| error.to_string())?;
        match attribute.key.local_name().as_ref() {
            "lat" => latitude = value.parse::<f64>().ok(),
            "lon" => longitude = value.parse::<f64>().ok(),
            _ => {}
        }
    }
    let latitude = latitude.ok_or_else(|| "GPX point is missing latitude".to_string())?;
    let longitude = longitude.ok_or_else(|| "GPX point is missing longitude".to_string())?;
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return Err("GPX point contains invalid coordinates".into());
    }
    Ok(PendingPoint {
        latitude,
        longitude,
        ..PendingPoint::default()
    })
}

pub fn closest(
    points: &[TrackPoint],
    epoch_millis: i64,
    maximum_seconds: u64,
) -> Result<(&TrackPoint, f64), String> {
    let insertion = points
        .binary_search_by_key(&epoch_millis, |point| point.epoch_millis)
        .unwrap_or_else(|index| index);
    let before = insertion.checked_sub(1).and_then(|index| points.get(index));
    let after = points.get(insertion);
    let selected = match (before, after) {
        (Some(left), Some(right)) => {
            if (epoch_millis - left.epoch_millis).abs() <= (right.epoch_millis - epoch_millis).abs()
            {
                left
            } else {
                right
            }
        }
        (Some(point), None) | (None, Some(point)) => point,
        (None, None) => return Err("No GPX points are available".into()),
    };

    let difference_millis = (selected.epoch_millis - epoch_millis).abs();
    if difference_millis > maximum_seconds.saturating_mul(1000) as i64 {
        return Err(format!(
            "Closest GPX point is {:.1} minutes away",
            difference_millis as f64 / 60_000.0
        ));
    }

    for point in points
        .iter()
        .filter(|point| point.epoch_millis.cmp(&selected.epoch_millis) == Ordering::Equal)
    {
        if (point.latitude - selected.latitude).abs() >= 0.0000001
            || (point.longitude - selected.longitude).abs() >= 0.0000001
        {
            return Err("Conflicting GPX locations share the same timestamp".into());
        }
    }
    Ok((selected, difference_millis as f64 / 1000.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earlier_point_wins_an_exact_tie() {
        let points = vec![
            TrackPoint {
                epoch_millis: 0,
                latitude: 1.0,
                longitude: 1.0,
                altitude: None,
                source: "a".into(),
            },
            TrackPoint {
                epoch_millis: 2_000,
                latitude: 2.0,
                longitude: 2.0,
                altitude: None,
                source: "a".into(),
            },
        ];
        let (point, difference) = closest(&points, 1_000, 60).unwrap();
        assert_eq!(point.epoch_millis, 0);
        assert_eq!(difference, 1.0);
    }

    #[test]
    fn rejects_conflicting_equal_timestamps() {
        let points = vec![
            TrackPoint {
                epoch_millis: 0,
                latitude: 1.0,
                longitude: 1.0,
                altitude: None,
                source: "a".into(),
            },
            TrackPoint {
                epoch_millis: 0,
                latitude: 2.0,
                longitude: 2.0,
                altitude: None,
                source: "b".into(),
            },
        ];
        assert!(closest(&points, 0, 60).is_err());
    }
}
