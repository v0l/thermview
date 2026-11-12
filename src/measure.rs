//! Inclusive video-range measurements. Decode a few frames per UI update so
//! lengthy selections remain cancellable without retaining decoded rasters.
use crate::Error;
use crate::image::{Params, Spot, ThermalImage};
use crate::video::Video;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSpot {
    pub frame: usize,
    pub spot: Spot,
}

#[derive(Clone, Debug)]
pub struct Summary {
    pub start: usize,
    pub end: usize,
    pub frames: usize,
    pub hottest: FrameSpot,
    pub hottest_frames: Vec<FrameSpot>,
    pub coldest: FrameSpot,
    pub average_max: f64,
    pub average_min: f64,
    pub mean: f64,
    pub edited_calibration: bool,
}

#[derive(Default)]
pub struct Measurement {
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub result: Option<Summary>,
    pub error: Option<String>,
    pub hottest_rank: usize,
    job: Option<Job>,
}

struct Job {
    start: usize,
    end: usize,
    next: usize,
    params: Option<Params>,
    hottest: Option<FrameSpot>,
    hottest_frames: Vec<FrameSpot>,
    coldest: Option<FrameSpot>,
    maxima: f64,
    minima: f64,
    sum: f64,
    pixels: u64,
}

impl Measurement {
    pub fn mark_start(&mut self, frame: usize) {
        self.invalidate();
        self.start = Some(frame);
    }
    pub fn mark_end(&mut self, frame: usize) {
        self.invalidate();
        self.end = Some(frame);
    }
    pub fn invalidate(&mut self) {
        self.hottest_rank = 0;
        self.result = None;
        self.error = None;
        self.job = None;
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn cancel(&mut self) {
        self.job = None;
        self.error = Some("Analysis cancelled. Your range is still marked.".into());
    }
    pub fn running(&self) -> bool {
        self.job.is_some()
    }
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.job.as_ref().map(|j| (j.next - j.start, j.end - j.start + 1))
    }
    pub fn valid_range(&self) -> bool {
        matches!((self.start, self.end), (Some(start), Some(end)) if start <= end)
    }
    pub fn begin(&mut self, frame_count: usize, params: Option<Params>) {
        self.invalidate();
        let (Some(start), Some(end)) = (self.start, self.end) else {
            self.error = Some("Mark both a start and an end frame.".into());
            return;
        };
        if start > end || end >= frame_count {
            self.error = Some(
                "The end frame must be at or after the start frame and inside the video.".into(),
            );
            return;
        }
        self.job = Some(Job {
            start,
            end,
            next: start,
            params,
            hottest: None,
            hottest_frames: Vec::new(),
            coldest: None,
            maxima: 0.0,
            minima: 0.0,
            sum: 0.0,
            pixels: 0,
        });
    }
    pub fn advance(&mut self, video: &Video) {
        let Some(mut job) = self.job.take() else { return };
        #[cfg(not(target_arch = "wasm32"))]
        let started = std::time::Instant::now();
        for _ in 0..4 {
            let frame = match video.frame(job.next) {
                Ok(frame) => frame,
                Err(e) => {
                    self.error = Some(e.to_string());
                    return;
                }
            };
            let frame = match job.params {
                Some(params) => frame.recompute(params),
                None => frame,
            };
            if let Err(e) = job.push(&frame) {
                self.error = Some(e.to_string());
                return;
            }
            if job.next > job.end {
                self.result = Some(job.finish());
                return;
            }
            #[cfg(not(target_arch = "wasm32"))]
            if started.elapsed() >= std::time::Duration::from_millis(6) {
                break;
            }
        }
        self.job = Some(job);
    }
}

impl Job {
    fn push(&mut self, image: &ThermalImage) -> Result<(), Error> {
        let mut count = 0u64;
        for &value in &image.celsius {
            if value.is_finite() && value > -273.1 {
                self.sum += value as f64;
                count += 1;
            }
        }
        if count == 0 {
            return Err(Error::new(format!(
                "Frame {} contains no valid temperature readings; analysis stopped.",
                self.next + 1
            )));
        }
        self.pixels += count;
        let max = image.stats.max;
        let min = image.stats.min;
        if self.hottest.is_none_or(|p| max.celsius > p.spot.celsius) {
            self.hottest = Some(FrameSpot { frame: self.next, spot: max });
        }
        if self.coldest.is_none_or(|p| min.celsius < p.spot.celsius) {
            self.coldest = Some(FrameSpot { frame: self.next, spot: min });
        }
        self.hottest_frames.push(FrameSpot { frame: self.next, spot: max });
        self.maxima += max.celsius as f64;
        self.minima += min.celsius as f64;
        self.next += 1;
        Ok(())
    }
    fn finish(mut self) -> Summary {
        self.hottest_frames
            .sort_by(|a, b| b.spot.celsius.total_cmp(&a.spot.celsius).then(a.frame.cmp(&b.frame)));
        let frames = self.end - self.start + 1;
        Summary {
            start: self.start,
            end: self.end,
            frames,
            hottest: self.hottest.expect("at least one valid frame"),
            hottest_frames: self.hottest_frames,
            coldest: self.coldest.expect("at least one valid frame"),
            average_max: self.maxima / frames as f64,
            average_min: self.minima / frames as f64,
            mean: self.sum / self.pixels as f64,
            edited_calibration: self.params.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Format;

    #[test]
    fn summaries_distinguish_peak_averages_from_pixel_weighted_mean() {
        let mut job = Job {
            start: 10,
            end: 11,
            next: 10,
            params: None,
            hottest: None,
            hottest_frames: Vec::new(),
            coldest: None,
            maxima: 0.0,
            minima: 0.0,
            sum: 0.0,
            pixels: 0,
        };
        job.push(&ThermalImage::new(Format::FlirCsq, 2, 1, vec![10.0, 30.0])).unwrap();
        job.push(&ThermalImage::new(Format::FlirCsq, 3, 1, vec![20.0, 40.0, 60.0])).unwrap();
        let result = job.finish();
        assert_eq!(result.frames, 2);
        assert_eq!(
            (result.hottest.frame, result.hottest.spot.x, result.hottest.spot.celsius),
            (11, 2, 60.0)
        );
        assert_eq!((result.coldest.frame, result.coldest.spot.celsius), (10, 10.0));
        assert_eq!(result.average_max, 45.0);
        assert_eq!(result.average_min, 15.0);
        assert_eq!(result.mean, 32.0);
    }

    #[test]
    fn marking_and_cancelling_invalidate_work_and_reversed_ranges_are_rejected() {
        let mut measurement = Measurement::default();
        measurement.mark_start(8);
        measurement.mark_end(2);
        measurement.begin(10, None);
        assert!(measurement.error.is_some());
        assert!(!measurement.running());
        measurement.mark_end(8);
        measurement.begin(10, None);
        assert_eq!(measurement.progress(), Some((0, 1)));
        measurement.mark_start(7);
        assert!(!measurement.running());
        measurement.begin(10, None);
        measurement.cancel();
        assert!(!measurement.running());
        assert_eq!((measurement.start, measurement.end), (Some(7), Some(8)));
        measurement.clear();
        assert_eq!((measurement.start, measurement.end), (None, None));
    }

    #[test]
    fn invalid_pixels_are_excluded_and_empty_frames_are_rejected() {
        let mut job = Job {
            start: 0,
            end: 0,
            next: 0,
            params: None,
            hottest: None,
            hottest_frames: Vec::new(),
            coldest: None,
            maxima: 0.0,
            minima: 0.0,
            sum: 0.0,
            pixels: 0,
        };
        job.push(&ThermalImage::new(
            Format::FlirCsq,
            4,
            1,
            vec![f32::NAN, -273.15, 22.0, f32::INFINITY],
        ))
        .unwrap();
        let result = job.finish();
        assert_eq!((result.mean, result.average_max, result.average_min), (22.0, 22.0, 22.0));
        let mut job = Job {
            start: 0,
            end: 0,
            next: 0,
            params: None,
            hottest: None,
            hottest_frames: Vec::new(),
            coldest: None,
            maxima: 0.0,
            minima: 0.0,
            sum: 0.0,
            pixels: 0,
        };
        assert!(job.push(&ThermalImage::new(Format::FlirCsq, 1, 1, vec![f32::NAN])).is_err());
    }

    #[test]
    fn hottest_candidates_rank_distinct_frames_and_break_ties_chronologically() {
        let mut job = Job {
            start: 10,
            end: 13,
            next: 10,
            params: None,
            hottest: None,
            hottest_frames: Vec::new(),
            coldest: None,
            maxima: 0.0,
            minima: 0.0,
            sum: 0.0,
            pixels: 0,
        };
        for max in [30.0, 50.0, 50.0, 40.0] {
            job.push(&ThermalImage::new(Format::FlirCsq, 2, 1, vec![10.0, max])).unwrap();
        }
        let result = job.finish();
        assert_eq!(
            result.hottest_frames.iter().map(|p| p.frame).collect::<Vec<_>>(),
            vec![11, 12, 13, 10]
        );
        assert_eq!(result.hottest_frames[0], result.hottest);
        assert_eq!(result.hottest_frames[3].spot.celsius, 30.0);
        let mut measurement = Measurement::default();
        measurement.hottest_rank = 3;
        measurement.result = Some(result);
        measurement.mark_end(12);
        assert_eq!(measurement.hottest_rank, 0);
        assert!(measurement.result.is_none());
    }
}
