
const SPI_PIXEL_LUT: &[u16] = &[
0o4444, // 0
0o4446, // 1
0o4464, // 2
0o4466, // 3
0o4644, // 4
0o4646, // 5
0o4664, // 6
0o4666, // 7
0o6444, // 8
0o6446, // 9
0o6464, // A
0o6466, // B
0o6644, // C
0o6646, // D
0o6664, // E
0o6666, // F
];

pub const NUM_LEDS: usize = 90;
pub const TX_BUFFER_SIZE: usize = NUM_LEDS * 9 + 1;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgb {
	pub r: u8,
	pub g: u8,
	pub b: u8,
}

impl Rgb {
	pub const fn new(r: u8, g: u8, b: u8) -> Self {
		Self { r, g, b }
	}

	pub fn to_spi_bits(self) -> [u8; 9] {
		let r_hi = SPI_PIXEL_LUT[usize::from(self.r >> 4)];
		let r_lo = SPI_PIXEL_LUT[usize::from(self.r & 0xf)];

		let g_hi = SPI_PIXEL_LUT[usize::from(self.g >> 4)];
		let g_lo = SPI_PIXEL_LUT[usize::from(self.g & 0xf)];

		let b_hi = SPI_PIXEL_LUT[usize::from(self.b >> 4)];
		let b_lo = SPI_PIXEL_LUT[usize::from(self.b & 0xf)];

		[
			(g_hi >> 4).try_into().unwrap(),
			((g_hi << 4 & 0xff) | (g_lo >> 8)).try_into().unwrap(),
			(g_lo & 0xff).try_into().unwrap(),

			(r_hi >> 4).try_into().unwrap(), 
			((r_hi << 4 & 0xff) | (r_lo >> 8)).try_into().unwrap(),
			(r_lo & 0xff).try_into().unwrap(), 

			(b_hi >> 4).try_into().unwrap(),
			((b_hi << 4 & 0xff) | (b_lo >> 8)).try_into().unwrap(),
			(b_lo & 0xff).try_into().unwrap(),
		]
	}
}

pub fn fill_transfer_buffer(spi_buf: &mut [u8; TX_BUFFER_SIZE], leds: &[Rgb; NUM_LEDS]) {
    let mut idx = 0;

    for pix in leds {
        let buf = pix.to_spi_bits();
        spi_buf[idx..idx+9].copy_from_slice(&buf);
        idx += 9;
    }
    spi_buf[NUM_LEDS * 9] = 0;
}

// pub fn encode_led(col: Rgb) -> 