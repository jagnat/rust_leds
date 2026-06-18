use crate::util::*;
use crate::audio::*;

pub trait Sketch {
	fn render (&mut self, leds: &mut [Rgb; NUM_LEDS], p: &SketchParams);
}

pub struct SketchParams {
	pub dt: f32,
	pub t: f32,
	pub audio: &'a AudioFeatures,
}

pub struct LevelPulse {phase: f32}
impl Sketch for LevelPulse {
	fn render (&mut self, leds: &mut [Rgb; NUM_LEDS], p: &SketchParams) {
		
	}
}

pub fn draw() {
}
