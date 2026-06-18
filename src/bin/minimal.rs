#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use rust_leds as _; // global logger + panicking-behavior + memory layout

#[rtic::app(
	device = stm32f4xx_hal::pac,
	// TODO: Replace the `FreeInterrupt1, ...` with free interrupt vectors if software tasks are used
	// You can usually find the names of the interrupt vectors in the some_hal::pac::interrupt enum.
	// dispatchers = [FreeInterrupt1, ...]
)]
mod app {
	use rust_leds::util::NUM_LEDS;
	use rust_leds::util::TX_BUFFER_SIZE;
	use rust_leds::audio::*;
	use rust_leds::sketches::*;
	use stm32f4xx_hal::dma::MemoryToPeripheral;
	use stm32f4xx_hal::gpio::alt::rcc;
	use stm32f4xx_hal::i2s::stm32_i2s_v12x::driver::*;
use stm32f4xx_hal::prelude::*;
	use stm32f4xx_hal::gpio::*;
	use stm32f4xx_hal::spi::*;
	use stm32f4xx_hal::i2s::*;
	use stm32f4xx_hal::dma::*;
	use stm32f4xx_hal::dma::config::DmaConfig;
	use stm32f4xx_hal::pac::*;
	use rust_leds::util::*;
	use stm32f4xx_hal::rcc::*;
	use rtic_monotonics::systick::prelude::*;

	systick_monotonic!(Mono, 1000);

	// len is * 4 because each frame has 32 bit words
	// one stereo frame is 64 bytes as 4 u16: [L hi] [L lo] [R hi (0)] [R lo (0)]
	const AUDIO_FRAMES: usize = 1024;
	const AUDIO_BUF_LEN: usize = AUDIO_FRAMES * 4;

	// For pcm streaming to host, decimate 32k to 8k (decimate = 4) to stream ez
	const DECIMATE: usize = 4;
	const PCM_CHUNK: usize = AUDIO_FRAMES / DECIMATE;

	type TxTransfer = Transfer<
		Stream2<DMA2>,
		2,
		Tx<SPI1>,
		MemoryToPeripheral,
		&'static mut [u8; TX_BUFFER_SIZE]>;

	type RxTransfer = Transfer<
		Stream3<DMA1>,
		0,
		I2sDriver<I2s<SPI2>, Master, Receive, Philips>,
		PeripheralToMemory,
		&'static mut [u16; AUDIO_BUF_LEN]>;

	fn decode_left_sample(buf: &[u16; AUDIO_BUF_LEN], i: usize) -> i32 {
		let hi = buf[4 * i] as u32;
		let lo = buf[4 * i + 1] as u32;
		(((hi << 16) | lo) as i32) >> 8
	}

	#[shared]
	struct Shared {
		tx_transfer: TxTransfer,
		rx_transfer: RxTransfer,
		audio: AudioFeatures, 
	}

	#[local]
	struct Local {
		led: PC13<Output>,
		count: i32,
		// spare third buffer swapped in and out
		audio_buf: Option<&'static mut [u16; AUDIO_BUF_LEN]>,
	}

	#[init]
	fn init(cx: init::Context) -> (Shared, Local) {
		defmt::info!("init");

		let rcc = cx.device.RCC.constrain();
		let mut rcc = rcc.freeze(
			Config::hse(25.MHz())
				.sysclk(96.MHz())
				.i2s_clk(150.MHz()));

		Mono::start(cx.core.SYST, rcc.clocks.sysclk().to_Hz());
		let gpioa = cx.device.GPIOA.split(&mut rcc);
		let gpiob = cx.device.GPIOB.split(&mut rcc);
		let gpioc = cx.device.GPIOC.split(&mut rcc);

		let sck = gpioa.pa5.into_alternate::<5>().speed(Speed::VeryHigh);
		let mosi = gpioa.pa7.into_alternate::<5>().speed(Speed::VeryHigh);

		let mode = Mode {
			polarity: Polarity::IdleLow,
			phase: Phase::CaptureOnFirstTransition, };

		let spi = Spi::new(
			cx.device.SPI1,
			(Some(sck), SPI1::NoMiso, Some(mosi)),
			mode,
			3.MHz(),
			&mut rcc);

		// SPI1_TX -> DMA2 Stream2
		let stream = stm32f4xx_hal::dma::StreamsTuple::new(
			cx.device.DMA2,
			&mut rcc).2;

		let tx_buffer = cortex_m::singleton!(: [u8; TX_BUFFER_SIZE] = [0; TX_BUFFER_SIZE]).unwrap();
		fill_transfer_buffer(tx_buffer, &[Rgb::new(255, 0, 0); NUM_LEDS]);

		let tx = spi.use_dma().tx();

		let mut tx_transfer = Transfer::init_memory_to_peripheral(
			stream,
			tx,
			tx_buffer,
			None,
			DmaConfig::default().memory_increment(true).transfer_complete_interrupt(true));

		let i2s2_sd = gpiob.pb15.into_alternate::<5>().speed(Speed::VeryHigh);
		let i2s2_ck = gpiob.pb10.into_alternate::<5>().speed(Speed::VeryHigh);
		let i2s2_ws = gpiob.pb12.into_alternate::<5>().speed(Speed::VeryHigh);

		let i2s2 = I2s::new(
			cx.device.SPI2,
			(i2s2_ws, i2s2_ck, SPI2::NoMck, i2s2_sd),
			&mut rcc);
		let i2s2_cfg = I2sDriverConfig::new_master()
			.direction(Receive)
			.standard(Philips)
			.data_format(DataFormat::Data24Channel32)
			.request_frequency(32000/*32khz*/)
			.clock_polarity(ClockPolarity::IdleHigh);
		let mut i2s2_driver = I2sDriver::new(i2s2, i2s2_cfg);

		i2s2_driver.set_rx_dma(true);

		// SPI2_RX -> DMA1 Stream3, Channel 0
		let rx_stream = stm32f4xx_hal::dma::StreamsTuple::new(
			cx.device.DMA1,
			&mut rcc
		).3;

		// DOuble buffering with one spare
		let rx_buf_a = cortex_m::singleton!(: [u16; AUDIO_BUF_LEN] = [0; AUDIO_BUF_LEN]).unwrap();
		let rx_buf_b = cortex_m::singleton!(: [u16; AUDIO_BUF_LEN] = [0; AUDIO_BUF_LEN]).unwrap();
		let rx_buf_c = cortex_m::singleton!(: [u16; AUDIO_BUF_LEN] = [0; AUDIO_BUF_LEN]).unwrap();

		let mut rx_transfer = Transfer::init_peripheral_to_memory(
			rx_stream,
			i2s2_driver,
			rx_buf_a,
			Some(rx_buf_b),
			DmaConfig::default()
				.memory_increment(true)
				.transfer_complete_interrupt(true)
				.double_buffer(true));

		let led = gpioc.pc13.into_push_pull_output();

		tx_transfer.start(|_tx| {});
		rx_transfer.start(|i2s| i2s.enable());

		toggle_led::spawn().ok();
		render::spawn().ok();

		let audio = AudioFeatures::default();

		(
			Shared {
				tx_transfer,
				rx_transfer,
				audio,
			},
			Local {
				led,
				count,
				audio_buf: Some(rx_buf_c),
			},
		)
	}

	// Optional idle, can be removed if not needed.
	// #[idle]
	// fn idle(_: idle::Context) -> ! {
	//     defmt::info!("idle");

	//     loop {
	//         continue;
	//     }
	// }

	#[task(local = [led, count])]
	async fn toggle_led(cx: toggle_led::Context) {
		let led = cx.local.led;

		defmt::debug!("Test");

		loop {
			*cx.local.count += 1;
			led.toggle();
			Mono::delay(500.millis()).await;
		}
	}

	#[task(shared = [tx_transfer])]
	async fn render(mut cx: render::Context) {
		let mut next = Mono::now();
		let mut led_buf = [Rgb::new(0,0,0); NUM_LEDS];
		let mut g: u8 = 0;
		loop {
			next += (1000/120).millis();

			g = (g + 1) % (NUM_LEDS as u8);

			for (i, p) in led_buf.iter_mut().enumerate() {
				if (i + usize::from(g)) % 30 == 0 {
					*p = Rgb::new(0, 255, 0);
				} else {
					*p = Rgb::new(0, 0, 0);
				}
			}

			cx.shared.tx_transfer.lock(|transfer| {
				unsafe {
					let _ = transfer.next_transfer_with(|buf, _| {
						fill_transfer_buffer(buf, &led_buf);
						(buf, ())
					});
				}
			});
			
			Mono::delay_until(next).await;
		}
	}

	#[task(binds=DMA2_STREAM2, shared = [tx_transfer])]
	fn spi_complete(mut cx: spi_complete::Context) {
		cx.shared.tx_transfer.lock(|transfer| {
			transfer.clear_flags(DmaFlag::TransferComplete);
		});
	}

	#[task(binds = DMA1_STREAM3, priority = 2, shared = [rx_transfer, audio], local = [audio_buf, log_div: u32 = 0])]
	fn i2s_dma(mut cx: i2s_dma::Context) {
		let local = cx.local;
		let spare = local.audio_buf.take().unwrap();

		// swap our spare buffer into the DMA and take back the one that just filled
		let buffer = match cx.shared.rx_transfer.lock(|t| t.next_transfer(spare)) {
			Ok((done, _current)) => done,
			Err(DMAError::Overrun(b))
			| Err(DMAError::NotReady(b))
			| Err(DMAError::SmallBuffer(b)) => {
				*local.audio_buf = Some(b);
				return;
			}
		};
		
		// 'buffer' is now ours

		// Uncomment to decimate and then stream over RTT
		// let mut pcm = [0i16; PCM_CHUNK];
		// for (j, slot) in pcm.iter_mut().enumerate() {
		//	*slot = (decode_left_sample(buffer, j * DECIMATE) >> 8) as i16;
		// }
		//defmt::info!("PCM {=[?]}", &pcm[..]);

		// hand the buffer back
		*local.audio_buf = Some(buffer);
	}
}
