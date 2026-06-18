
pub const NUM_BANDS: usize = 6;

#[derive(Clone, Copy, Default)]
pub struct AudioFeatures {
	pub level: f32,
	pub bands: [f32; NUM_BANDS],
}


